package routes

import (
	"runtime"
	"sync"

	"github.com/vjeantet/ldapserver"
)

type session struct {
	password string
}

// connection number -> *session
var sessions sync.Map

func setSession(m *ldapserver.Message, s *session) {
	id := m.Client.Numero
	if _, loaded := sessions.Swap(id, s); !loaded {
		// Drop the entry once the library releases the client (connection gone).
		// Key by the int, not the pointer, so the map doesn't keep the client alive.
		runtime.AddCleanup(m.Client, func(id int) { sessions.Delete(id) }, id)
	}
}

func getSession(m *ldapserver.Message) (*session, bool) {
	v, ok := sessions.Load(m.Client.Numero)
	if !ok {
		return nil, false
	}
	return v.(*session), true
}

func clearSession(m *ldapserver.Message) {
	sessions.Delete(m.Client.Numero)
}
