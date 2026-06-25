import { defineCollection, z } from "astro:content";

const blogCollection = defineCollection({
  type: "content",
  schema: z.object({
    title: z.string(),
    subtitle: z.string().optional(),
    pubDate: z.date(),
    tags: z.array(z.string()).optional(),
    draft: z.boolean().optional(),
    thread: z.string().optional(),
    threadOrder: z.number().optional(),
  }),
});

export const collections = {
  blog: blogCollection,
};
