import { existsSync } from 'node:fs';
import { defineCollection } from 'astro:content';
import { glob } from 'astro/loaders';
import { z } from 'astro/zod';
import { cardPath } from './lib/assets';

const cardExists = (card: string) => existsSync(`./public${cardPath(card)}`);

const blog = defineCollection({
  loader: glob({
    base: './content',
    pattern: '{blog,drafts}/*.md',
    generateId: ({ entry }) => entry.replace(/^.*\//, '').replace(/\.md$/, ''),
  }),
  schema: z
    .object({
      title: z.string(),
      pubDate: z.coerce.date(),
      subtitle: z.string().optional(),
      description: z.string().optional(),
      card: z
        .string()
        .refine(cardExists, {
          error: (issue) =>
            `public${cardPath(String(issue.input))} does not exist`,
        })
        .optional(),
    })
    .strict(),
});

export const collections = { blog };
