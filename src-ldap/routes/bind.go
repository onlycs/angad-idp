package routes

import (
	"errors"
	"fmt"
	"log/slog"

	"angad.page/transit/proto"
	ldap "github.com/vjeantet/ldapserver"
)

func Bind(w ldap.ResponseWriter, m *ldap.Message) {
	r := m.GetBindRequest()
	res := ldap.NewBindResponse(ldap.LDAPResultOperationsError)

	if r.AuthenticationChoice() != "simple" {
		res.SetResultCode(ldap.LDAPResultAuthMethodNotSupported)
		res.SetDiagnosticMessage("only simple bind is supported")
		w.Write(res)
		return
	}

	res_err := proto.RouteCheckPassword(api, proto.CheckPasswordRequest{
		UnknownIdentity: string(r.Name()),
		Password:        r.AuthenticationSimple().String(),
	})

	if res_err == nil {
		res.SetResultCode(ldap.LDAPResultSuccess)
		res.SetDiagnosticMessage("")
	}

	if transport, ok := errors.AsType[proto.CheckPasswordFfiErrorTransit](res_err); ok {
		slog.Warn(fmt.Sprint("Transit error:\n", transport.Source.Report()))
		res.SetResultCode(ldap.LDAPResultOperationsError)
		res.SetDiagnosticMessage("Internal server error")
	}

	if error, ok := errors.AsType[proto.CheckPasswordFfiErrorProtocol](res_err); ok {
		if no_user, ok := errors.AsType[proto.CheckPasswordErrorNoUser](error); ok {
			res.SetResultCode(ldap.LDAPResultNoSuchObject)
			res.SetDiagnosticMessage(no_user.Source.Display())
		}

		if incorrect, ok := errors.AsType[proto.CheckPasswordErrorIncorrectPassword](error); ok {
			res.SetResultCode(ldap.LDAPResultInvalidCredentials)
			res.SetDiagnosticMessage(incorrect.Source.Display())
		}
	}

	w.Write(res)
}
