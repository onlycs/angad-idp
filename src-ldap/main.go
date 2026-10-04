package main

import (
	"fmt"

	"angad.page/transit/idp_proto"
	"angad.page/transit/transit_core"
)

func main() {
	transit_core.BeginLogging(transit_core.LogLevelInfo)

	tr, err := transit_core.Connect(transit_core.TransitOptions{
		Connect: transit_core.ConnectOptions{
			Addr: "127.0.0.1",
			Port: 23849,
			Crt:  nil,
		},
	})

	if err != nil {
		fmt.Printf("Idk bro ive never written go before: \n%v", err.Report())
	}

	_, autherr := idp_proto.RouteAuthenticate(tr, idp_proto.AuthenticateRequest{
		Query: idp_proto.UserQueryUid{
			Field0: "id",
		},
		Password: "joe",
	})

	if autherr != nil {
		fmt.Printf("Authentication failed: %v\n", autherr.Report())
	}
}
