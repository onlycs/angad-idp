package routes

import (
	"errors"
	"fmt"
	"log/slog"

	"angad.page/transit/proto"
	"github.com/go-ldap/ldap/v3"
	"github.com/vjeantet/ldapserver"
)

func Bind(w ldapserver.ResponseWriter, m *ldapserver.Message) {
	clearSession(m)

	r := m.GetBindRequest()
	res := ldapserver.NewBindResponse(ldapserver.LDAPResultOperationsError)

	if r.AuthenticationChoice() != "simple" {
		res.SetResultCode(ldapserver.LDAPResultAuthMethodNotSupported)
		res.SetDiagnosticMessage("only simple bind is supported")
		w.Write(res)
		return
	}

	dn, err := ldap.ParseDN(string(r.Name()))

	if err != nil {
		res.SetResultCode(ldapserver.LDAPResultInvalidDNSyntax)
		w.Write(res)
		return
	}

	name := string(dn.RDNs[0].Attributes[0].Value)

	if name == "$service" {
		pass := r.AuthenticationSimple().String()
		res_err := proto.RouteIntegrationAuthorize(Api, proto.IntegrationAuthorizeRequest{
			Key:   pass,
			Level: proto.AccessLevelReadWrite,
		})

		if res_err == nil {
			setSession(m, &session{password: pass})
			res.SetResultCode(ldapserver.LDAPResultSuccess)
		} else {
			res.SetResultCode(ldapserver.LDAPResultInvalidCredentials)
		}

		w.Write(res)
		return
	}

	res_err := proto.RouteCheckPassword(Api, proto.CheckPasswordRequest{
		UnknownIdentity: name,
		Password:        r.AuthenticationSimple().String(),
	})

	if res_err == nil {
		res.SetResultCode(ldapserver.LDAPResultSuccess)

		w.Write(res)
		return
	}

	if transport, ok := errors.AsType[proto.CheckPasswordFfiErrorTransit](res_err); ok {
		slog.Warn(fmt.Sprint("Transit error:\n", transport.Source.Report()))
		res.SetResultCode(ldapserver.LDAPResultOperationsError)
		res.SetDiagnosticMessage("Internal server error")
	}

	if error, ok := errors.AsType[proto.CheckPasswordFfiErrorProtocol](res_err); ok {
		if no_user, ok := errors.AsType[proto.CheckPasswordErrorNoUser](error); ok {
			res.SetResultCode(ldapserver.LDAPResultNoSuchObject)
			res.SetDiagnosticMessage(no_user.Source.Display())
		}

		if incorrect, ok := errors.AsType[proto.CheckPasswordErrorIncorrectPassword](error); ok {
			res.SetResultCode(ldapserver.LDAPResultInvalidCredentials)
			res.SetDiagnosticMessage(incorrect.Source.Display())
		}
	}

	w.Write(res)
}
