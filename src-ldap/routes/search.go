package routes

import (
	"fmt"
	"log/slog"
	"slices"
	"strings"

	"angad.page/transit/proto"
	"github.com/go-ldap/ldap/v3"
	"github.com/lor00x/goldap/message"
	"github.com/vjeantet/ldapserver"
)

func Search(w ldapserver.ResponseWriter, m *ldapserver.Message) {
	req := m.GetSearchRequest()
	res := ldapserver.NewSearchResultDoneResponse(ldap.LDAPResultSuccess)
	defer func() { w.Write(res) }()

	dn, err := ldap.ParseDN(string(req.BaseObject()))
	if err != nil {
		res.SetResultCode(ldapserver.LDAPResultInvalidDNSyntax)
		return
	}

	rdns := dn.RDNs
	n := len(rdns)

	if n < 2 || n > 4 || !VerifyDc(dn) {
		return
	}

	// ensure all rdns before cn/uid/mail attr is single-attributed
	for _, rdn := range rdns[n/4:] { // if n == 4 this is 1: otherwise 0:
		if len(rdn.Attributes) != 1 {
			res.SetResultCode(ldapserver.LDAPResultNotAllowedOnRDN)
			return
		}
	}

	userPreds := []func(proto.User) bool{}
	groupPreds := []func(Group) bool{}

	// ensure scope makes sense
	scope := req.Scope()
	if scope == ldap.ScopeChildren {
		scope = ldap.ScopeWholeSubtree
	}

	switch n {
	case 2:
		if scope != ldap.ScopeWholeSubtree {
			return
		}
	case 3:
		if scope == ldap.ScopeBaseObject {
			return
		}
	case 4:
		if scope == ldap.ScopeSingleLevel {
			return
		}
	}

	if n >= 3 {
		ou := rdns[n-3].Attributes[0]
		if !VerifyOuAndDc(dn, "users") && !VerifyOuAndDc(dn, "groups") {
			return
		}

		switch strings.ToLower(ou.Value) {
		case "groups":
			userPreds = append(userPreds, func(proto.User) bool {
				return false
			})

			if n == 4 {
				if len(rdns[0].Attributes) != 1 {
					res.SetResultCode(ldapserver.LDAPResultNotAllowedOnRDN)
					return
				}

				attr := rdns[0].Attributes[0]
				groupPreds = append(groupPreds, func(g Group) bool {
					return matchGroup(&g, attr.Type, attr.Value)
				})
			}

		case "users":
			groupPreds = append(groupPreds, func(Group) bool {
				return false
			})

			if n == 4 {
				for _, attr := range rdns[0].Attributes {
					userPreds = append(userPreds, func(u proto.User) bool {
						return matchUser(&u, attr.Type, attr.Value)
					})
				}
			}
		}
	}

	users, eusers := proto.RouteUserList(Api, proto.AuthenticationIntegration{
		Field0: Integration,
	})

	if _, handle, ret := HandleError[
		proto.UserListFfiErrorTransit,
		proto.UserListFfiErrorProtocol,
	](eusers, (*message.LDAPResult)(&res)); handle {
		if !ret {
			slog.Warn(INTEGRATION_INVALID)
			WriteISE((*message.LDAPResult)(&res))
		}
		return
	}

	preds := filtersApply(req.Filter())
	userPreds = append(userPreds, preds.user)
	groupPreds = append(groupPreds, preds.group)

	// Collect matching entries.
	groups := buildGroups(&users)

	validUsers := []proto.User{}
	validGroups := []Group{}

	for _, user := range users {
		if slices.ContainsFunc(userPreds, func(pred func(proto.User) bool) bool {
			return !pred(user)
		}) {
			continue
		}

		validUsers = append(validUsers, user)
	}

	for _, group := range groups {
		if slices.ContainsFunc(groupPreds, func(pred func(Group) bool) bool {
			return !pred(group)
		}) {
			continue
		}
		validGroups = append(validGroups, group)
	}

	attrs := []message.LDAPString(req.Attributes())

	if len(attrs) == 0 || slices.Contains(attrs, "*") {
		attrs = []message.LDAPString{
			"uid", "cn", "mail", "objectClass", "memberOf", "member",
		}
	}

	for _, user := range validUsers {
		ent := ldapserver.NewSearchResultEntry(fmt.Sprint(
			"uid=", user.Id,
			",ou=users,dc=angad,dc=page",
		))

		for _, attr := range attrs {
			vals := returnValuesUser(&user, message.AttributeDescription(attr))
			if len(vals) == 0 {
				continue
			}

			ent.AddAttribute(message.AttributeDescription(attr), vals...)
		}

		w.Write(ent)
	}

	for _, group := range validGroups {
		ent := ldapserver.NewSearchResultEntry(fmt.Sprint(
			"cn=", membershipToString(group.toShip()),
			",ou=groups,dc=angad,dc=page",
		))

		for _, attr := range attrs {
			vals := returnValuesGroup(&group, message.AttributeDescription(attr))
			if len(vals) == 0 {
				continue
			}

			ent.AddAttribute(message.AttributeDescription(attr), vals...)
		}

		w.Write(ent)
	}
}

type Group struct {
	slug    string
	admin   bool
	members []*proto.User
}

func (self *Group) toShip() proto.Membership {
	return proto.Membership{
		Admin: self.admin,
		Slug:  self.slug,
	}
}

func membershipToString(ship proto.Membership) string {
	var sadm string
	if ship.Admin {
		sadm = proto.GroupSuffixAdmin()
	} else {
		sadm = proto.GroupSuffixUser()
	}

	return fmt.Sprint(ship.Slug, ":", sadm)
}

func membershipFromString(s string) proto.Membership {
	slug, suffix, ok := strings.Cut(s, ":")
	if !ok {
		return proto.Membership{Slug: slug, Admin: false}
	}

	return proto.Membership{
		Slug:  slug,
		Admin: suffix == proto.GroupSuffixAdmin(),
	}
}

func buildGroups(users *[]proto.User) []Group {
	mapped := make(map[string][]*proto.User)

	for i := range *users {
		u := &(*users)[i]

		for _, m := range u.Memberships {
			s := membershipToString(m)
			mapped[s] = append(mapped[s], u)
		}
	}

	res := make([]Group, 0, len(mapped))
	for k, v := range mapped {
		ship := membershipFromString(k)
		res = append(res, Group{
			slug:    ship.Slug,
			admin:   ship.Admin,
			members: v,
		})
	}

	return res
}

type Preds struct {
	user  func(u proto.User) bool
	group func(g Group) bool
}

func (self Preds) and(other Preds) Preds {
	oldUser := self.user
	self.user = func(u proto.User) bool {
		return oldUser(u) && other.user(u)
	}

	oldGroup := self.group
	self.group = func(g Group) bool {
		return oldGroup(g) && other.group(g)
	}

	return self
}

func (self Preds) or(other Preds) Preds {
	oldUser := self.user
	self.user = func(u proto.User) bool {
		return oldUser(u) || other.user(u)
	}

	oldGroup := self.group
	self.group = func(g Group) bool {
		return oldGroup(g) || other.group(g)
	}

	return self
}

func (self Preds) not() Preds {
	oldUser := self.user
	self.user = func(u proto.User) bool {
		return !oldUser(u)
	}

	oldGroup := self.group
	self.group = func(g Group) bool {
		return !oldGroup(g)
	}

	return self
}

func dnEqual(a, b string) bool {
	da, err1 := ldap.ParseDN(a)
	db, err2 := ldap.ParseDN(b)
	if err1 != nil || err2 != nil {
		return strings.EqualFold(a, b)
	}
	return da.EqualFold(db)
}

func matchUserCustom[K ~string](self *proto.User, ldapKey K, matches func(s string) bool) bool {
	_ldapKey := string(ldapKey)
	nrm := NormalizeKey(_ldapKey)
	var anyOf []string

	if nrm == nil {
		return false
	}

	switch *nrm {
	case "uid":
		anyOf = []string{self.Id}
	case "cn":
		anyOf = []string{self.Username}
	case "mail":
		anyOf = []string{self.Email}
	case "objectClass":
		anyOf = []string{
			"top", "2.5.6.0",
			"person", "2.5.6.6",
			"organizationalperson", "2.5.6.7",
			"inetorgperson", "2.16.840.1.113730.3.2.2",
		}
	case "memberOf":
		anyOf = make([]string, 0, len(self.Memberships))

		for _, ship := range self.Memberships {
			anyOf = append(anyOf, fmt.Sprint(
				"cn=", membershipToString(ship),
				",ou=groups,dc=angad,dc=page",
			))
		}
	}

	return slices.ContainsFunc(anyOf, matches)
}

func matchUser[K ~string, T ~string](self *proto.User, ldapKey K, matchTo T) bool {
	_matchTo := string(matchTo)

	return matchUserCustom(self, ldapKey, func(s string) bool {
		return strings.EqualFold(s, _matchTo) || dnEqual(s, _matchTo)
	})
}

func matchGroupCustom[K ~string](self *Group, ldapKey K, matches func(s string) bool) bool {
	_ldapKey := string(ldapKey)
	nrm := NormalizeKey(_ldapKey)
	var anyOf []string

	if nrm == nil {
		return false
	}

	switch *nrm {
	case "cn":
		anyOf = []string{membershipToString(self.toShip())}
	case "objectClass":
		anyOf = []string{
			"top", "2.5.6.0",
			"groupOfNames", "2.5.6.9",
		}
	case "member":
		anyOf = make([]string, 0, len(self.members))

		for _, member := range self.members {
			anyOf = append(anyOf, fmt.Sprint(
				"uid=", member.Id,
				",ou=users,dc=angad,dc=page",
			))
		}
	}

	return slices.ContainsFunc(anyOf, matches)
}

func matchGroup[K ~string, T ~string](self *Group, ldapKey K, matchTo T) bool {
	_matchTo := string(matchTo)

	return matchGroupCustom(self, ldapKey, func(s string) bool {
		return strings.EqualFold(s, _matchTo) || dnEqual(s, _matchTo)
	})
}

func returnValuesUser(self *proto.User, ldapKey message.AttributeDescription) []message.AttributeValue {
	nrm := NormalizeKey(string(ldapKey))

	if nrm == nil {
		return []message.AttributeValue{}
	}

	switch *nrm {
	case "uid":
		return []message.AttributeValue{message.AttributeValue(self.Id)}
	case "cn":
		return []message.AttributeValue{message.AttributeValue(self.Username)}
	case "mail":
		return []message.AttributeValue{message.AttributeValue(self.Email)}
	case "objectClass":
		return []message.AttributeValue{
			"top",
			"person",
			"organizationalPerson",
			"inetOrgPerson",
		}
	case "memberOf":
		res := make([]message.AttributeValue, 0, len(self.Memberships))

		for _, ship := range self.Memberships {
			res = append(res, message.AttributeValue(fmt.Sprint(
				"cn=", membershipToString(ship),
				",ou=groups,dc=angad,dc=page",
			)))
		}

		return res
	default:
		return []message.AttributeValue{}
	}
}

func returnValuesGroup(self *Group, ldapKey message.AttributeDescription) []message.AttributeValue {
	nrm := NormalizeKey(string(ldapKey))

	if nrm == nil {
		return []message.AttributeValue{}
	}

	switch *nrm {
	case "cn":
		return []message.AttributeValue{message.AttributeValue(membershipToString(self.toShip()))}
	case "objectClass":
		return []message.AttributeValue{
			"top",
			"groupOfNames",
		}
	case "member":
		res := make([]message.AttributeValue, 0, len(self.members))

		for _, member := range self.members {
			res = append(res, message.AttributeValue(fmt.Sprint(
				"uid=", member.Id,
				",ou=users,dc=angad,dc=page",
			)))
		}

		return res
	default:
		return []message.AttributeValue{}
	}
}

func matchSubstrings(s string, subs []message.Substring) bool {
	s = strings.ToLower(s)

	for _, substr := range subs {
		switch sub := substr.(type) {
		case message.SubstringInitial:
			value := strings.ToLower(string(message.AssertionValue(sub)))
			if !strings.HasPrefix(s, value) {
				return false
			}
			s = s[len(value):]

		case message.SubstringAny:
			value := strings.ToLower(string(message.AssertionValue(sub)))
			pos := strings.Index(s, value)
			if pos == -1 {
				return false
			}
			s = s[pos+len(value):]

		case message.SubstringFinal:
			value := strings.ToLower(string(message.AssertionValue(sub)))
			if !strings.HasSuffix(s, value) {
				return false
			}

		default:
			return false
		}
	}

	return true
}

func filtersApply(filter message.Filter) Preds {
	var assertion message.AttributeValueAssertion

	switch f := filter.(type) {
	case message.FilterAnd:
		res := Preds{
			user:  func(u proto.User) bool { return true },
			group: func(g Group) bool { return true },
		}

		for _, and := range f {
			res = res.and(filtersApply(and))
		}

		return res

	case message.FilterOr:
		res := Preds{
			user:  func(u proto.User) bool { return false },
			group: func(g Group) bool { return false },
		}

		for _, or := range f {
			res = res.or(filtersApply(or))
		}

		return res

	case message.FilterNot:
		return filtersApply(f.Filter).not()

	case message.FilterApproxMatch:
		assertion = message.AttributeValueAssertion(f)
		goto equality

	case message.FilterEqualityMatch:
		assertion = message.AttributeValueAssertion(f)
		goto equality

	case message.FilterSubstrings:
		key := f.Type_()
		subs := f.Substrings()

		return Preds{
			user: func(u proto.User) bool {
				return matchUserCustom(&u, key, func(s string) bool {
					return matchSubstrings(s, subs)
				})
			},
			group: func(g Group) bool {
				return matchGroupCustom(&g, key, func(s string) bool {
					return matchSubstrings(s, subs)
				})
			},
		}

	case message.FilterPresent:
		key := message.AttributeDescription(f)

		return Preds{
			func(u proto.User) bool {
				return matchUserCustom(&u, key, func(s string) bool { return true })
			},
			func(g Group) bool {
				return matchGroupCustom(&g, key, func(s string) bool { return true })
			},
		}

	default:
		return Preds{
			user:  func(u proto.User) bool { return false },
			group: func(g Group) bool { return false },
		}
	}

equality:
	key := assertion.AttributeDesc()
	value := assertion.AssertionValue()

	return Preds{
		user: func(u proto.User) bool {
			return matchUser(&u, key, value)
		},
		group: func(g Group) bool {
			return matchGroup(&g, key, value)
		},
	}
}
