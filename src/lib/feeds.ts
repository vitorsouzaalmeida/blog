import type { RootContent } from 'hast';
import { fromHtml } from 'hast-util-from-html';
import { toHtml } from 'hast-util-to-html';

const absoluteNode = <Node extends RootContent>(node: Node, postUrl: string): Node =>
  node.type === 'element'
    ? {
        ...node,
        properties: Object.fromEntries(
          Object.entries(node.properties).map(([attribute, value]) => [
            attribute,
            ['href', 'src', 'poster'].includes(attribute) && typeof value === 'string'
              ? new URL(value, postUrl).href
              : value,
          ]),
        ),
        children: node.children.map((child) => absoluteNode(child, postUrl)),
      }
    : node;

export const absoluteContent = (html: string, postUrl: string): string => {
  const tree = fromHtml(html, { fragment: true });
  return toHtml({ ...tree, children: tree.children.map((node) => absoluteNode(node, postUrl)) });
};
