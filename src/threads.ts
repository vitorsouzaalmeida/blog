import { getCollection, type CollectionEntry } from "astro:content";

export interface Thread {
  id: string;
  title: string;
  description: string;
}

// A thread groups a sequence of standalone posts into one continuous story.
// Each post opts in with `thread: <id>` in its frontmatter; order follows
// `threadOrder` when set, otherwise `pubDate` ascending (reading order).
export const THREADS: Thread[] = [
  {
    id: "isolated-env",
    title: "isolated work environment",
    description:
      "Building a disposable, isolated Linux environment and to work.",
  },
];

export const getThread = (id: string): Thread | undefined =>
  THREADS.find((t) => t.id === id);

// Published parts of a thread, in reading order (oldest first).
export async function getThreadParts(
  id: string,
): Promise<CollectionEntry<"blog">[]> {
  const posts = await getCollection("blog", ({ data }) => !data.draft);
  return posts
    .filter((post) => post.data.thread === id)
    .sort(
      (a, b) =>
        (a.data.threadOrder ?? Infinity) - (b.data.threadOrder ?? Infinity) ||
        Number(a.data.pubDate) - Number(b.data.pubDate),
    );
}

export interface ThreadPlacement {
  thread: Thread;
  index: number; // 1-based position within the thread
  total: number;
}

// Maps each post slug to its thread placement, but only for threads with at
// least two published parts — a lone "part 1 of 1" stays a plain post until
// the next part ships, so nothing half-formed leaks to the list.
export async function buildThreadMap(): Promise<Map<string, ThreadPlacement>> {
  const map = new Map<string, ThreadPlacement>();
  for (const thread of THREADS) {
    const parts = await getThreadParts(thread.id);
    if (parts.length < 2) continue;
    parts.forEach((post, i) => {
      map.set(post.slug, { thread, index: i + 1, total: parts.length });
    });
  }
  return map;
}
