"use strict";

const call = (fn, ...args) =>
  new Promise((resolve, reject) => {
    fn(...args, (err, value) => (err ? reject(err) : resolve(value)));
  });

async function loadProfile(db, id) {
  const user = await call(db.getUser.bind(db), id);
  if (!user) throw new Error("user not found");
  const posts = await call(db.getPosts.bind(db), user.id);
  const counted = await Promise.all(
    posts.map(async (post) => {
      const comments = await call(db.getComments.bind(db), post.id);
      return { id: post.id, title: post.title, commentCount: comments.length };
    }),
  );
  return { user, posts: counted };
}

module.exports = { loadProfile };
