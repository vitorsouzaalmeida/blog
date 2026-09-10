import rss, { type RSSFeedItem } from '@astrojs/rss';
import type { APIRoute } from 'astro';
import { absoluteContent } from '../lib/feeds';
import { getPosts, postUrl, summary, type Post } from '../lib/posts';
import { site } from '../site';

const feedItem = (post: Post): RSSFeedItem => {
  if (!post.rendered)
    throw new Error(`Missing rendered content for ${post.id}`);
  const url = new URL(postUrl(post.id), site.url).href;
  return {
    title: post.data.title,
    pubDate: post.data.pubDate,
    link: url,
    description: summary(post),
    content: absoluteContent(post.rendered.html, url),
  };
};

export const GET: APIRoute = async () =>
  rss({
    title: site.title,
    description: site.description,
    site: site.url,
    items: (await getPosts()).map(feedItem),
  });
