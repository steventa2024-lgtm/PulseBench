import { useState } from "react";

export interface SearchableListProps {
  items: string[];
  placeholder?: string;
  onSelect?: (item: string) => void;
}

export function SearchableList({ items, placeholder = "Search...", onSelect }: SearchableListProps) {
  const [query, setQuery] = useState("");
  const needle = query.trim().toLowerCase();
  const visible = items
    .map((item, index) => ({ item, index }))
    .filter(({ item }) => needle === "" || item.toLowerCase().includes(needle));

  return (
    <div>
      <input aria-label="search" placeholder={placeholder} value={query} onChange={(e) => setQuery(e.target.value)} />
      {query !== "" && (
        <button aria-label="clear" onClick={() => setQuery("")}>
          ×
        </button>
      )}
      <p data-testid="count">
        {visible.length} of {items.length} shown
      </p>
      {visible.length === 0 && items.length > 0 ? (
        <p data-testid="empty">No results for "{query.trim()}"</p>
      ) : visible.length > 0 ? (
        <ul>
          {visible.map(({ item, index }) => (
            <li key={index} onClick={() => onSelect?.(item)}>
              {item}
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  );
}
