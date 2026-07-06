import { useState, useEffect } from 'react'
import './index.css'
import Feed from './components/Feed'
import AuthModal from './components/AuthModal'

function App() {
  const [token, setToken] = useState(localStorage.getItem('token'))
  const [user, setUser] = useState(null)
  const [showAuth, setShowAuth] = useState(false)

  useEffect(() => {
    if (token) {
      fetch('/api/users/me', {
        headers: { 'Authorization': `Bearer ${token}` }
      })
      .then(res => {
        if (res.ok) return res.json();
        throw new Error('Token invalid');
      })
      .then(data => setUser(data))
      .catch(() => {
        setToken(null);
        localStorage.removeItem('token');
      });
    }
  }, [token]);

  const handleLogout = () => {
    setToken(null);
    setUser(null);
    localStorage.removeItem('token');
  };

  return (
    <>
      <nav className="navbar">
        <h1>Nexus</h1>
        <div>
          {user ? (
            <div style={{ display: 'flex', alignItems: 'center', gap: '16px' }}>
              <span>Hello, <strong>{user.username}</strong></span>
              <button className="secondary" onClick={handleLogout}>Logout</button>
            </div>
          ) : (
            <button onClick={() => setShowAuth(true)}>Log in / Sign up</button>
          )}
        </div>
      </nav>

      <main className="container">
        <Feed token={token} user={user} />
      </main>

      {showAuth && (
        <AuthModal 
          onClose={() => setShowAuth(false)}
          onSuccess={(t) => {
            setToken(t);
            localStorage.setItem('token', t);
            setShowAuth(false);
          }} 
        />
      )}
    </>
  )
}

export default App
