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

	rusers, eusers := proto.RouteUserList(api, proto.AuthenticationIntegration{
		Field0: integration,
	})

	if _, handle, ret := HandleError[
		proto.UserListFfiErrorTransit,
		proto.UserListFfiErrorProtocol,
	](eusers, message.LDAPResult(res)); handle {
		if !ret {
			slog.Warn(INTEGRATION_INVALID)
			WriteISE(message.LDAPResult(res))
		}
		return
	}

	dn, err := ldap.ParseDN(string(req.BaseObject()))
	if err != nil {
		res.SetResultCode(ldapserver.LDAPResultInvalidDNSyntax)
		return
	}

	rdns := dn.RDNs
	n := len(rdns)

	if n < 2 || n > 4 {
		res.SetResultCode(ldapserver.LDAPResultInvalidDNSyntax)
		return
	}

	// Validate the domain suffix.
	for _, rdn := range rdns[1:] {
		if len(rdn.Attributes) != 1 {
			res.SetResultCode(ldapserver.LDAPResultInvalidDNSyntax)
			return
		}
	}

	base := rdns[n-1].Attributes[0]
	second := rdns[n-2].Attributes[0]

	if strings.ToLower(base.Type) != "dc" || strings.ToLower(base.Value) != "page" ||
		strings.ToLower(second.Type) != "dc" || strings.ToLower(second.Value) != "angad" {
		return
	}

	userPreds := []func(proto.User) bool{}
	groupPreds := []func(Group) bool{}

	// Validate the requested scope.
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
		if scope != ldap.ScopeSingleLevel && scope != ldap.ScopeWholeSubtree {
			return
		}
	case 4:
		if scope == ldap.ScopeSingleLevel || scope == ldap.ScopeChildren {
			return
		}
	}

	if n >= 3 {
		ou := rdns[n-3].Attributes[0]
		if strings.ToLower(ou.Type) != "ou" || (strings.ToLower(ou.Value) != "users" && strings.ToLower(ou.Value) != "groups") {
			res.SetResultCode(ldapserver.LDAPResultInvalidDNSyntax)
			return
		}

		switch strings.ToLower(ou.Value) {
		case "groups":
			userPreds = append(userPreds, func(proto.User) bool {
				return false
			})

			if n == 4 {
				if len(rdns[0].Attributes) != 1 {
					res.SetResultCode(ldapserver.LDAPResultInvalidDNSyntax)
					return
				}

				attr := rdns[0].Attributes[0]
				if attr.Type != "cn" {
					return
				}

				groupPreds = append(groupPreds, func(g Group) bool {
					return appToString(g.slug, g.admin) == attr.Value
				})
			}

		case "users":
			groupPreds = append(groupPreds, func(Group) bool {
				return false
			})

			if n == 4 {
				for _, attr := range rdns[0].Attributes {
					userPreds = append(userPreds, func(u proto.User) bool {
						return slices.Contains(
							applicableValuesUser[string](&u, message.AttributeDescription(attr.Type)),
							strings.ToLower(attr.Value),
						)
					})
				}
			}
		}
	}

	preds := filtersApply(req.Filter())
	userPreds = append(userPreds, preds.user)
	groupPreds = append(groupPreds, preds.group)

	// Collect matching entries.
	groups := buildGroups(&rusers)

	validUsers := []proto.User{}
	validGroups := []Group{}

	for _, user := range rusers {
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
			vals := returnValuesUser[message.AttributeValue](&user, message.AttributeDescription(attr))
			if len(vals) == 0 {
				continue
			}

			ent.AddAttribute(message.AttributeDescription(attr), vals...)
		}

		w.Write(ent)
	}

	for _, group := range validGroups {
		ent := ldapserver.NewSearchResultEntry(fmt.Sprint(
			"cn=", appToString(group.slug, group.admin),
			",ou=groups,dc=angad,dc=page",
		))

		for _, attr := range attrs {
			vals := returnValuesGroup[message.AttributeValue](&group, message.AttributeDescription(attr))
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

func appToString(slug string, adm bool) string {
	var sadm string
	if adm {
		sadm = proto.GroupSuffixAdmin()
	} else {
		sadm = proto.GroupSuffixUser()
	}

	return fmt.Sprint(slug, ":", sadm)
}

func stringToApp(s string) (string, bool) {
	slug, suffix, ok := strings.Cut(s, ":")
	if !ok {
		return s, false
	}

	return slug, suffix == proto.GroupSuffixAdmin()
}

func buildGroups(users *[]proto.User) []Group {
	mapped := make(map[string][]*proto.User)

	for i := range *users {
		u := &(*users)[i]

		for _, m := range u.Memberships {
			s := appToString(m.Slug, m.Admin)
			mapped[s] = append(mapped[s], u)
		}
	}

	res := make([]Group, 0, len(mapped))
	for k, v := range mapped {
		slug, adm := stringToApp(k)
		res = append(res, Group{
			slug:    slug,
			admin:   adm,
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

func applicableValuesUser[T ~string](self *proto.User, ldapKey message.AttributeDescription) []T {
	switch strings.ToLower(string(ldapKey)) {
	case "uid", "0.9.2342.19200300.100.1.1":
		return []T{T(self.Id)}
	case "cn", "2.5.4.3":
		return []T{T(self.Username)}
	case "mail", "0.9.2342.19200300.100.1.3":
		return []T{T(self.Email)}
	case "objectclass", "2.5.4.0":
		return []T{
			"top", "2.5.6.0",
			"person", "2.5.6.6",
			"organizationalperson", "2.5.6.7",
			"inetorgperson", "2.16.840.1.113730.3.2.2",
		}
	case "memberof", "1.2.840.113556.1.2.102":
		res := make([]T, 0, len(self.Memberships))

		for _, ship := range self.Memberships {
			res = append(res, T(fmt.Sprint(
				"cn=", appToString(ship.Slug, ship.Admin),
				",ou=groups,dc=angad,dc=page",
			)))
		}

		return res
	default:
		return []T{}
	}
}

func returnValuesUser[T ~string](self *proto.User, ldapKey message.AttributeDescription) []T {
	switch strings.ToLower(string(ldapKey)) {
	case "uid", "0.9.2342.19200300.100.1.1":
		return []T{T(self.Id)}
	case "cn", "2.5.4.3":
		return []T{T(self.Username)}
	case "mail", "0.9.2342.19200300.100.1.3":
		return []T{T(self.Email)}
	case "objectclass", "2.5.4.0":
		return []T{
			"top",
			"person",
			"organizationalPerson",
			"inetOrgPerson",
		}
	case "memberof", "1.2.840.113556.1.2.102":
		res := make([]T, 0, len(self.Memberships))

		for _, ship := range self.Memberships {
			res = append(res, T(fmt.Sprint(
				"cn=", appToString(ship.Slug, ship.Admin),
				",ou=groups,dc=angad,dc=page",
			)))
		}

		return res
	default:
		return []T{}
	}
}

func applicableValuesGroup[T ~string](self *Group, ldapKey message.AttributeDescription) []T {
	switch strings.ToLower(string(ldapKey)) {
	case "cn", "2.5.4.3":
		return []T{T(appToString(self.slug, self.admin))}
	case "objectclass", "2.5.4.0":
		return []T{
			"top", "2.5.6.0",
			"groupofnames", "2.5.6.9",
		}
	case "member", "2.5.4.31":
		res := make([]T, 0, len(self.members))

		for _, member := range self.members {
			res = append(res, T(fmt.Sprint(
				"uid=", member.Id,
				",ou=users,dc=angad,dc=page",
			)))
		}

		return res
	default:
		return []T{}
	}
}

func returnValuesGroup[T ~string](self *Group, ldapKey message.AttributeDescription) []T {
	switch strings.ToLower(string(ldapKey)) {
	case "cn", "2.5.4.3":
		return []T{T(appToString(self.slug, self.admin))}
	case "objectclass", "2.5.4.0":
		return []T{
			"top",
			"groupOfNames",
		}
	case "member", "2.5.4.31":
		res := make([]T, 0, len(self.members))

		for _, member := range self.members {
			res = append(res, T(fmt.Sprint(
				"uid=", member.Id,
				",ou=users,dc=angad,dc=page",
			)))
		}

		return res
	default:
		return []T{}
	}
}

func matches(s string, subs []message.Substring) bool {
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
				return slices.ContainsFunc(applicableValuesUser[string](&u, key), func(s string) bool {
					return matches(s, subs)
				})
			},
			group: func(g Group) bool {
				return slices.ContainsFunc(applicableValuesGroup[string](&g, key), func(s string) bool {
					return matches(s, subs)
				})
			},
		}

	case message.FilterPresent:
		key := message.AttributeDescription(f)

		return Preds{
			func(u proto.User) bool {
				return len(applicableValuesUser[string](&u, key)) > 0
			},
			func(g Group) bool {
				return len(applicableValuesGroup[string](&g, key)) > 0
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
	value := string(assertion.AssertionValue())
	eq := func(s string) bool { return strings.EqualFold(s, value) }

	return Preds{
		user: func(u proto.User) bool {
			return slices.ContainsFunc(applicableValuesUser[string](&u, key), eq)
		},
		group: func(g Group) bool {
			return slices.ContainsFunc(applicableValuesGroup[string](&g, key), eq)
		},
	}
}
