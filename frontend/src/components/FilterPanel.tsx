'use client';

import { FilterState, SortOption, Category, TagWithCount, Production } from '@/lib/types';

interface FilterPanelProps {
  filters: FilterState;
  categories: Category[];
  tags: TagWithCount[];
  productions: Production[];
  onFilterChange: (filters: Partial<FilterState>) => void;
  onClearFilters: () => void;
}

export default function FilterPanel({
  filters,
  categories,
  tags,
  productions,
  onFilterChange,
  onClearFilters,
}: FilterPanelProps) {
  const sortOptions: { value: SortOption; label: string }[] = [
    { value: 'date_desc', label: 'Date (Newest First)' },
    { value: 'date_asc', label: 'Date (Oldest First)' },
    { value: 'name_asc', label: 'Name (A-Z)' },
    { value: 'name_desc', label: 'Name (Z-A)' },
    { value: 'duration_desc', label: 'Duration (Longest First)' },
    { value: 'duration_asc', label: 'Duration (Shortest First)' },
    { value: 'size_desc', label: 'Size (Largest First)' },
    { value: 'size_asc', label: 'Size (Smallest First)' },
  ];

  const handleTagToggle = (tagName: string) => {
    const newTags = filters.tags.includes(tagName)
      ? filters.tags.filter((t) => t !== tagName)
      : [...filters.tags, tagName];
    onFilterChange({ tags: newTags });
  };

  const hasActiveFilters =
    filters.category ||
    filters.tags.length > 0 ||
    filters.production !== null ||
    filters.dateFrom ||
    filters.dateTo;

  return (
    <div className="space-y-6">
      {/* Sort */}
      <div>
        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
          Sort By
        </label>
        <select
          value={filters.sort}
          onChange={(e) => onFilterChange({ sort: e.target.value as SortOption })}
          className="input"
        >
          {sortOptions.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>
      </div>

      {/* Category Filter */}
      <div>
        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
          Category
        </label>
        <select
          value={filters.category}
          onChange={(e) => onFilterChange({ category: e.target.value })}
          className="input"
        >
          <option value="">All Categories</option>
          {categories.map((cat) => (
            <option key={cat.name} value={cat.name}>
              {cat.name} ({cat.count})
            </option>
          ))}
        </select>
      </div>

      {/* Production Filter */}
      <div>
        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
          Production
        </label>
        <select
          value={filters.production ?? ''}
          onChange={(e) =>
            onFilterChange({ production: e.target.value ? Number(e.target.value) : null })
          }
          className="input"
        >
          <option value="">All Productions</option>
          {productions.map((prod) => (
            <option key={prod.id} value={prod.id}>
              {prod.title} ({prod.video_count ?? 0})
            </option>
          ))}
        </select>
      </div>

      {/* Date Range Filter */}
      <div>
        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
          Date Range
        </label>
        <div className="space-y-2">
          <div>
            <label className="block text-xs text-gray-600 dark:text-gray-400 mb-1">
              From
            </label>
            <input
              type="date"
              value={filters.dateFrom ? filters.dateFrom.toISOString().split('T')[0] : ''}
              onChange={(e) =>
                onFilterChange({
                  dateFrom: e.target.value ? new Date(e.target.value) : null,
                })
              }
              className="input text-sm"
            />
          </div>
          <div>
            <label className="block text-xs text-gray-600 dark:text-gray-400 mb-1">
              To
            </label>
            <input
              type="date"
              value={filters.dateTo ? filters.dateTo.toISOString().split('T')[0] : ''}
              onChange={(e) =>
                onFilterChange({
                  dateTo: e.target.value ? new Date(e.target.value) : null,
                })
              }
              className="input text-sm"
            />
          </div>
        </div>
      </div>

      {/* Tags Filter */}
      <div>
        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
          Tags
        </label>
        <div className="max-h-64 overflow-y-auto space-y-2 border border-gray-300 dark:border-gray-600 rounded-lg p-3">
          {tags.length === 0 ? (
            <p className="text-sm text-gray-500 dark:text-gray-400 italic">
              No tags available
            </p>
          ) : (
            tags.map((tag) => (
              <label
                key={tag.id}
                className="flex items-center space-x-2 cursor-pointer hover:bg-gray-50 dark:hover:bg-gray-700 p-1 rounded"
              >
                <input
                  type="checkbox"
                  checked={filters.tags.includes(tag.name)}
                  onChange={() => handleTagToggle(tag.name)}
                  className="rounded border-gray-300 text-primary-600 focus:ring-primary-500"
                />
                <span className="text-sm text-gray-700 dark:text-gray-300 flex-1">
                  {tag.name}
                </span>
                <span className="text-xs text-gray-500 dark:text-gray-400">
                  {tag.count}
                </span>
              </label>
            ))
          )}
        </div>
      </div>

      {/* Clear Filters */}
      {hasActiveFilters && (
        <button
          onClick={onClearFilters}
          className="w-full btn btn-secondary text-sm"
        >
          Clear All Filters
        </button>
      )}
    </div>
  );
}
