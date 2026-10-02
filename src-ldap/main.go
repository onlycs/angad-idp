package main

import (
	"angad.page/transit/transit_core"
)

func main() {
	_, err := transit_core.Connect(transit_core.TransitOptions{
		Connect: transit_core.ConnectOptions{
			Addr: "127.0.0.1",
			Port: 397,
			Crt:  nil,
		},
	})

	if err != nil {
		err.Tag()
	}
}
