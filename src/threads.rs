use std::collections::HashMap;

use crate::content::Post;

#[derive(Debug)]
pub struct Thread {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
}

pub static THREADS: &[Thread] = &[Thread {
    id: "isolated-env",
    title: "isolated work environment",
    description: "Building a disposable, isolated Linux environment and to work.",
}];

pub fn get_thread(id: &str) -> Option<&'static Thread> {
    THREADS.iter().find(|t| t.id == id)
}

pub fn thread_parts<'a>(posts: &'a [Post], id: &str) -> Vec<&'a Post> {
    let mut parts: Vec<&Post> = posts
        .iter()
        .filter(|p| p.thread.as_deref() == Some(id))
        .collect();
    parts.sort_by_key(|p| (p.thread_order.unwrap_or(i64::MAX), p.pub_date));
    parts
}

#[derive(Debug)]
pub struct Placement {
    pub thread: &'static Thread,
    pub index: usize,
}

pub fn thread_map(posts: &[Post]) -> HashMap<&str, Placement> {
    THREADS
        .iter()
        .flat_map(|t| {
            let parts = thread_parts(posts, t.id);
            let multi = parts.len() >= 2;
            parts
                .into_iter()
                .enumerate()
                .filter(move |_| multi)
                .map(move |(i, p)| {
                    (
                        p.slug.as_str(),
                        Placement {
                            thread: t,
                            index: i + 1,
                        },
                    )
                })
        })
        .collect()
}

pub struct ThreadNav<'a> {
    pub thread: &'static Thread,
    pub index: usize,
    pub total: usize,
    pub prev: Option<&'a Post>,
    pub next: Option<&'a Post>,
}

pub fn thread_nav<'a>(post: &Post, posts: &'a [Post]) -> Option<ThreadNav<'a>> {
    let thread = get_thread(post.thread.as_deref()?)?;
    let parts = thread_parts(posts, thread.id);
    let i = parts.iter().position(|p| p.slug == post.slug)?;
    (parts.len() >= 2).then(|| ThreadNav {
        thread,
        index: i + 1,
        total: parts.len(),
        prev: i.checked_sub(1).and_then(|j| parts.get(j)).copied(),
        next: parts.get(i + 1).copied(),
    })
}
