import { getCollection, type CollectionEntry } from 'astro:content';

export type Post = Readonly<CollectionEntry<'blog'>>;

const compareText = (a: string, b: string) => (a < b ? -1 : a > b ? 1 : 0);

const comparePosts = (a: Post, b: Post) =>
  b.data.pubDate.getTime() - a.data.pubDate.getTime() ||
  compareText(a.id, b.id);

export const sortPosts = (posts: readonly Post[]): readonly Post[] =>
  posts.toSorted(comparePosts);

export const getPosts = async (): Promise<readonly Post[]> =>
  sortPosts(await getCollection('blog'));

export const postUrl = (id: string) => `/blog/${id}/`;

export const isoDate = (date: Date) => date.toISOString().slice(0, 10);
const monthName = new Intl.DateTimeFormat('en-GB', {
  month: 'long',
  timeZone: 'UTC',
});

export const displayDate = (date: Date) =>
  `${date.getUTCDate()} ${monthName.format(date)}, ${date.getUTCFullYear()}`;
export const summary = (post: Post) =>
  post.data.description ?? post.data.subtitle;
