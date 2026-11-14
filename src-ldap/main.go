package main

import (
	"log"
	"os"
	"os/signal"
	"syscall"

	"angad.page/idp-ldap/routes"
	"angad.page/transit/transit"
	ldap "github.com/vjeantet/ldapserver"
)

func main() {
	listen_addr := routes.EnvOrDefault("LISTEN_ADDRESS", "0.0.0.0")
	listen_port := routes.EnvOrDefault("LISTEN_PORT", "389")

	transit.BeginLogging(transit.LogLevelInfo)
	ldap.Logger = log.New(os.Stdout, "[server] ", log.LstdFlags)

	routes.Setup()
	server := ldap.NewServer()
	router := ldap.NewRouteMux()

	router.Bind(routes.Bind)
	router.Search(routes.Search)

	server.Handle(router)
	go server.ListenAndServe(listen_addr + ":" + listen_port)

	ch := make(chan os.Signal, 8)
	signal.Notify(ch, syscall.SIGINT, syscall.SIGTERM)
	<-ch
	close(ch)

	server.Stop()
}
