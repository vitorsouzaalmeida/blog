export type Post = {
  id: string;
  slug: string;
  body: string;
  collection: string;
  data: {
    title: string;
    pubDate: Date;
    tags: string[];
  };
  render: () => Promise<unknown>;
};
