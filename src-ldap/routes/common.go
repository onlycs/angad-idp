package routes

import (
	"errors"
	"fmt"
	"log/slog"
	"os"
	"strconv"
	"strings"

	"angad.page/transit/transit"
	"github.com/go-ldap/ldap/v3"
	"github.com/lor00x/goldap/message"
	"github.com/vjeantet/ldapserver"
)

var Api *transit.Transit = nil
var Integration = os.Getenv("INTEGRATION_KEY")

const INTEGRATION_INVALID = "Permission denied, is INTEGRATION_KEY valid?"

func Setup() {
	transit_addr := os.Getenv("TRANSIT_ADDRESS")
	env_transit_port := os.Getenv("TRANSIT_PORT")
	transit_port, conv_err := strconv.Atoi(env_transit_port)

	if conv_err != nil {
		panic(conv_err)
	}

	var tls transit.Tls = transit.TlsInternal{}
	l_api, transit_err := transit.Connect(transit.TransitOptions{
		Connect: transit.ConnectOptions{
			Addr: transit_addr,
			Port: uint16(transit_port),
			Crt:  &tls,
		},
	})

	if transit_err != nil {
		panic(transit_err)
	}

	Api = l_api
}

func HandleError[
	T interface {
		~struct{ Source *transit.RouteError }
		error
	},
	P error,
](
	err error,
	res *message.LDAPResult,
) (*P, bool, bool) {
	if err == nil {
		return nil, false, false
	}

	if transport, ok := errors.AsType[T](err); ok {
		src := struct{ Source *transit.RouteError }(transport).Source
		slog.Warn("Transit error:\n" + src.Report())
	} else if p, ok := errors.AsType[P](err); ok {
		return &p, true, false
	} else {
		slog.Error("Unexpected error", "error", err)
	}

	WriteISE(res)

	return nil, true, true
}

var _alternates map[string][]string = map[string][]string{
	"dc":   {"dc", "domainComponent", "0.9.2342.19200300.100.1.25"},
	"ou":   {"ou", "organizationalUnitName", "2.5.4.11"},
	"uid":  {"uid", "0.9.2342.19200300.100.1.1"},
	"cn":   {"cn", "commonName", "2.5.4.3"},
	"mail": {"mail", "0.9.2342.19200300.100.1.3"},

	"objectClass": {"objectClass", "2.5.4.0"},
	"member":      {"member", "2.5.4.31"},
	"memberOf":    {"memberOf", "1.2.840.113556.1.2.102"},
}

var _normalized map[string]string = (func() map[string]string {
	res := make(map[string]string)

	for key, vals := range _alternates {
		for _, val := range vals {
			res[strings.ToLower(val)] = key
		}
	}

	return res
})()

func AlternativeKeys(standard string) []string {
	if res, ok := _alternates[standard]; ok {
		return res
	} else {
		return []string{}
	}
}

func NormalizeKey(unknown string) *string {
	if res, ok := _normalized[strings.ToLower(unknown)]; ok {
		return &res
	} else {
		return nil
	}
}

func WriteISE(res *message.LDAPResult) {
	res.SetResultCode(ldapserver.LDAPResultOperationsError)
	res.SetDiagnosticMessage("Internal server error")
}

func VerifyDc(dn *ldap.DN) bool {
	base, _ := ldap.ParseDN("dc=angad,dc=page")
	return dn.EqualFold(base) || base.AncestorOfFold(dn)
}

func VerifyOuAndDc(dn *ldap.DN, wants string) bool {
	base, _ := ldap.ParseDN(fmt.Sprint("ou=", wants, ",dc=angad,dc=page"))
	return dn.EqualFold(base) || base.AncestorOfFold(dn)
}
