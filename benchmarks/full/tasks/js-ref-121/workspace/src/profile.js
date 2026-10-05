"use strict";

/**
 * Load a user together with their posts and a comment count per post.
 * `db` has Node-style callback methods: getUser(id, cb), getPosts(userId, cb), getComments(postId, cb).
 * Result: { user, posts: [{ id, title, commentCount }] } (posts keep their original order).
 * Errors: Error("user not found") when the user does not exist; any db error is passed through unchanged.
 */
function loadProfile(db, id, callback) {
  db.getUser(id, (err, user) => {
    if (err) return callback(err);
    if (!user) return callback(new Error("user not found"));
    db.getPosts(user.id, (err, posts) => {
      if (err) return callback(err);
      const result = new Array(posts.length);
      let pending = posts.length;
      let failed = false;
      if (pending === 0) return callback(null, { user, posts: [] });
      posts.forEach((post, index) => {
        db.getComments(post.id, (err, comments) => {
          if (failed) return;
          if (err) {
            failed = true;
            return callback(err);
          }
          result[index] = { id: post.id, title: post.title, commentCount: comments.length };
          pending -= 1;
          if (pending === 0) callback(null, { user, posts: result });
        });
      });
    });
  });
}

module.exports = { loadProfile };
