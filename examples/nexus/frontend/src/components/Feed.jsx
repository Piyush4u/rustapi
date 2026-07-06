import { useState, useEffect } from 'react';

export default function Feed({ token, user }) {
  const [posts, setPosts] = useState([]);
  const [newPost, setNewPost] = useState('');

  const fetchPosts = async () => {
    const res = await fetch('/api/posts');
    const data = await res.json();
    setPosts(data);
  };

  useEffect(() => {
    fetchPosts();
  }, []);

  const handlePostSubmit = async (e) => {
    e.preventDefault();
    if (!newPost.trim() || !token) return;

    await fetch('/api/posts', {
      method: 'POST',
      headers: { 
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${token}`
      },
      body: JSON.stringify({ content: newPost })
    });
    setNewPost('');
    fetchPosts();
  };

  const handleCommentSubmit = async (postId, content) => {
    if (!token || !content.trim()) return;
    await fetch(`/api/posts/${postId}/comments`, {
      method: 'POST',
      headers: { 
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${token}`
      },
      body: JSON.stringify({ content })
    });
    fetchPosts();
  };

  return (
    <div>
      {user ? (
        <form onSubmit={handlePostSubmit} className="glass-panel" style={{ marginBottom: '32px' }}>
          <textarea 
            placeholder="What's on your mind?" 
            value={newPost}
            onChange={e => setNewPost(e.target.value)}
            rows={3}
            required
          />
          <div style={{ display: 'flex', justifyContent: 'flex-end' }}>
            <button type="submit">Post to Nexus</button>
          </div>
        </form>
      ) : (
        <div className="glass-panel" style={{ textAlign: 'center', marginBottom: '32px' }}>
          <p>Please log in to share your thoughts.</p>
        </div>
      )}

      <div>
        {posts.map(post => (
          <div key={post.id} className="post glass-panel">
            <div className="post-header">
              <div className="avatar">
                {post.author.username.charAt(0).toUpperCase()}
              </div>
              <div>
                <div className="author-name">{post.author.username}</div>
                <div className="post-date">
                  {new Date(post.created_at).toLocaleString()}
                </div>
              </div>
            </div>
            
            <div className="post-content">
              {post.content}
            </div>

            <div className="comments-section">
              {post.comments.map(c => (
                <div key={c.id} className="comment">
                  <div className="author-name">{c.author.username}</div>
                  <p>{c.content}</p>
                </div>
              ))}
              
              {user && (
                <CommentForm onSubmit={(content) => handleCommentSubmit(post.id, content)} />
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function CommentForm({ onSubmit }) {
  const [content, setContent] = useState('');

  const handleSubmit = (e) => {
    e.preventDefault();
    onSubmit(content);
    setContent('');
  };

  return (
    <form onSubmit={handleSubmit} style={{ display: 'flex', gap: '8px', marginTop: '12px' }}>
      <input 
        type="text" 
        placeholder="Write a comment..." 
        value={content}
        onChange={e => setContent(e.target.value)}
        style={{ marginBottom: 0 }}
      />
      <button type="submit" className="secondary">Reply</button>
    </form>
  );
}
