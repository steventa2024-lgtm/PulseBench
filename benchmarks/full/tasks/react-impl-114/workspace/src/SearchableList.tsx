export interface SearchableListProps {
  items: string[];
  placeholder?: string;
  /** Called with the clicked item. */
  onSelect?: (item: string) => void;
}

/**
 * A text input above a list of items.
 *
 *  - The input has `aria-label="search"` and the given placeholder (default "Search...").
 *  - Typing filters the items: case-insensitive "contains" match on the trimmed query,
 *    keeping the original order. An empty or blank query shows every item.
 *  - Each item is an <li> inside one <ul>; clicking an <li> calls `onSelect(item)`.
 *  - A <p data-testid="count"> shows "N of M shown" (N visible, M total).
 *  - When nothing matches, no <ul> is rendered; instead a <p data-testid="empty"> shows
 *    `No results for "<query>"` using the trimmed query.
 *  - A <button aria-label="clear"> is rendered only while the query is non-empty; clicking it
 *    empties the input.
 *  - Duplicate item strings must all be rendered and must not produce React key warnings.
 */
export function SearchableList(props: SearchableListProps) {
  return null;
}
