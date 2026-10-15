package routes

import (
	"errors"
	"log/slog"
	"os"
	"reflect"
	"strconv"

	"angad.page/transit/transit"
	"github.com/lor00x/goldap/message"
	ldap "github.com/vjeantet/ldapserver"
)

var api *transit.Transit = nil
var integration = os.Getenv("INTEGRATION_KEY")

const INTEGRATION_INVALID = "Permission denied, is INTEGRATION_KEY valid?"

func Setup() {
	transit_addr := os.Getenv("TRANSIT_ADDRESS")
	env_transit_port := os.Getenv("TRANSIT_PORT")
	transit_port, conv_err := strconv.Atoi(env_transit_port)

	if conv_err != nil {
		panic(conv_err)
	}

	l_api, transit_err := transit.Connect(transit.TransitOptions{
		Connect: transit.ConnectOptions{
			Addr: transit_addr,
			Port: uint16(transit_port),
		},
	})

	if transit_err != nil {
		panic(transit_err)
	}

	api = l_api
}

func HandleError[T error, P error](
	err error,
	res message.LDAPResult,
) (*P, bool, bool) {
	if err == nil {
		return nil, false, false
	}

	if transport, ok := errors.AsType[T](err); ok {
		v := reflect.ValueOf(transport)

		if v.Kind() == reflect.Pointer {
			v = v.Elem()
		}

		source := v.FieldByName("Source").
			Interface().(*transit.RouteError)

		slog.Warn("Transit error:\n" + source.Report())
	} else if p, ok := errors.AsType[P](err); ok {
		return &p, true, false
	} else {
		slog.Error("Unexpected error", "error", err)
	}

	WriteISE(res)

	return nil, true, true
}

func WriteISE(res message.LDAPResult) {
	res.SetResultCode(ldap.LDAPResultOperationsError)
	res.SetDiagnosticMessage("Internal server error")
}
