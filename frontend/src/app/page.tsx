'use client';

import { useState, useEffect } from 'react';
import { useRouter } from 'next/navigation';
import SearchBar from '@/components/SearchBar';
import FilterPanel from '@/components/FilterPanel';
import VideoGrid from '@/components/VideoGrid';
import VideoModal from '@/components/VideoModal';
import BulkActions from '@/components/BulkActions';
import Scanner from '@/components/Scanner';
import ProductionManager from '@/components/ProductionManager';
import { FilterState, Video, Production } from '@/lib/types';
import { getConfig, getVideos, getCategories, getTags, getProductions, getStatistics, rescanDirectory, semanticSearchVideos } from '@/lib/api';

type SearchMode = 'keyword' | 'semantic';

/** How many ranked results a semantic query returns. */
const SEMANTIC_LIMIT = 48;

export default function HomePage() {
  const router = useRouter();
  const [isLoading, setIsLoading] = useState(true);
  const [isConfigured, setIsConfigured] = useState(false);
  const [videos, setVideos] = useState<Video[]>([]);
  const [totalVideos, setTotalVideos] = useState(0);
  const [totalPages, setTotalPages] = useState(0);
  const [categories, setCategories] = useState<any[]>([]);
  const [tags, setTags] = useState<any[]>([]);
  const [productions, setProductions] = useState<Production[]>([]);
  const [selectedVideo, setSelectedVideo] = useState<Video | null>(null);
  const [selectedVideoIds, setSelectedVideoIds] = useState<Set<number>>(new Set());
  const [rescanScanId, setRescanScanId] = useState<string | null>(null);
  const [showProductionManager, setShowProductionManager] = useState(false);

  // Semantic search (natural-language ranking) — separate from the instant
  // keyword filter so we only hit the embedding API on explicit submit.
  const [searchMode, setSearchMode] = useState<SearchMode>('keyword');
  const [semanticHasRun, setSemanticHasRun] = useState(false);
  const [semanticIndexEmpty, setSemanticIndexEmpty] = useState(false);
  const [semanticWeak, setSemanticWeak] = useState(false);
  const [semanticError, setSemanticError] = useState<string | null>(null);

  const [filters, setFilters] = useState<FilterState>({
    search: '',
    category: '',
    tags: [],
    production: null,
    dateFrom: null,
    dateTo: null,
    orientation: '',
    sort: 'date_desc',
  });

  const [pagination, setPagination] = useState({
    page: 1,
    limit: 24,
  });

  // Check configuration on mount
  useEffect(() => {
    checkConfig();
  }, []);

  // Load data when filters or pagination change. Semantic mode is driven by
  // explicit submit (Enter), so we skip the auto-load there.
  useEffect(() => {
    if (isConfigured && searchMode === 'keyword') {
      loadData();
    }
  }, [filters, pagination, isConfigured, searchMode]);

  // Load categories and tags on mount
  useEffect(() => {
    if (isConfigured) {
      loadFilters();
    }
  }, [isConfigured]);

  const checkConfig = async () => {
    try {
      const config = await getConfig();
      if (!config.configured) {
        router.push('/setup');
      } else {
        setIsConfigured(true);
      }
    } catch (error) {
      console.error('Failed to check config:', error);
      // Redirect to setup on error (backend not available or not configured)
      router.push('/setup');
    }
  };

  const loadData = async () => {
    setIsLoading(true);
    try {
      const response = await getVideos(filters, pagination);
      setVideos(response.videos);
      setTotalVideos(response.total);
      setTotalPages(response.pages);
    } catch (error) {
      console.error('Failed to load videos:', error);
    } finally {
      setIsLoading(false);
    }
  };

  const loadFilters = async () => {
    try {
      const [categoriesData, tagsData, productionsData] = await Promise.all([
        getCategories(),
        getTags(),
        getProductions(),
      ]);
      setCategories(categoriesData);
      setTags(tagsData);
      setProductions(productionsData);
    } catch (error) {
      console.error('Failed to load filters:', error);
    }
  };

  const handleFilterChange = (newFilters: Partial<FilterState>) => {
    setFilters({ ...filters, ...newFilters });
    setPagination({ ...pagination, page: 1 }); // Reset to first page
  };

  // Search box text changed. In keyword mode this drives the instant filter; in
  // semantic mode it only updates the query text (the search runs on Enter).
  const handleSearchChange = (search: string) => {
    if (searchMode === 'semantic') {
      setFilters({ ...filters, search });
    } else {
      handleFilterChange({ search });
    }
  };

  // Run a semantic (meaning-based) search for the current query.
  const runSemanticSearch = async (query: string) => {
    const q = query.trim();
    if (!q) return;
    setIsLoading(true);
    setSemanticError(null);
    try {
      const res = await semanticSearchVideos(q, SEMANTIC_LIMIT);
      setVideos(res.videos);
      setTotalVideos(res.total);
      setTotalPages(1);
      setSemanticIndexEmpty(res.index_empty);
      setSemanticWeak(res.weak_match && !res.index_empty && res.videos.length > 0);
      setSemanticHasRun(true);
    } catch (err: any) {
      setSemanticError(err.message || 'Semantic search failed');
      setVideos([]);
      setTotalVideos(0);
      setSemanticHasRun(true);
    } finally {
      setIsLoading(false);
    }
  };

  // Flip between the instant keyword filter and semantic (AI) ranking.
  const handleToggleSemantic = () => {
    const next: SearchMode = searchMode === 'semantic' ? 'keyword' : 'semantic';
    setSearchMode(next);
    setSemanticError(null);
    setSemanticIndexEmpty(false);
    setSemanticWeak(false);
    setSemanticHasRun(false);
    if (next === 'semantic') {
      // Clear the keyword grid; results appear once the user submits a query.
      setVideos([]);
      setTotalVideos(0);
      setTotalPages(1);
    }
    // Switching back to keyword lets the load effect refresh the grid.
  };

  const handleClearFilters = () => {
    setFilters({
      search: '',
      category: '',
      tags: [],
      production: null,
      dateFrom: null,
      dateTo: null,
      orientation: '',
      sort: 'date_desc',
    });
    setPagination({ ...pagination, page: 1 });
  };

  const handleVideoSelect = (videoId: number) => {
    const newSelected = new Set(selectedVideoIds);
    if (newSelected.has(videoId)) {
      newSelected.delete(videoId);
    } else {
      newSelected.add(videoId);
    }
    setSelectedVideoIds(newSelected);
  };

  const handleClearSelection = () => {
    setSelectedVideoIds(new Set());
  };

  const handlePageChange = (newPage: number) => {
    setPagination({ ...pagination, page: newPage });
    window.scrollTo({ top: 0, behavior: 'smooth' });
  };

  const handleRescan = async () => {
    try {
      const response = await rescanDirectory();
      setRescanScanId(response.scan_id);
    } catch (error: any) {
      console.error('Failed to start rescan:', error);
    }
  };

  const handleRescanComplete = () => {
    setRescanScanId(null);
    loadData();
    loadFilters();
  };

  if (!isConfigured) {
    return (
      <div className="min-h-screen flex items-center justify-center">
        <div className="text-center">
          <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-primary-600 mx-auto mb-4" />
          <p className="text-gray-600 dark:text-gray-400">Loading...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen">
      {/* Header */}
      <header className="bg-white dark:bg-gray-800 shadow-sm sticky top-0 z-30">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-4">
          <div className="flex items-center justify-between mb-4">
            <img
              src="/logo.png"
              alt="Video Log Manager"
              className="h-16"
            />
            <div className="flex items-center gap-4">
              <button
                onClick={() => setShowProductionManager(true)}
                className="btn btn-secondary text-sm"
                title="Manage productions"
              >
                Productions
              </button>
              <button
                onClick={handleRescan}
                disabled={!!rescanScanId}
                className="btn btn-secondary text-sm"
                title="Rescan video directory for new videos"
              >
                {rescanScanId ? 'Scanning...' : 'Rescan'}
              </button>
              <button
                onClick={() => router.push('/settings')}
                className="btn btn-secondary text-sm"
                title="Settings"
              >
                Settings
              </button>
              <div className="text-sm text-gray-600 dark:text-gray-400">
                {totalVideos} video{totalVideos !== 1 ? 's' : ''}
              </div>
            </div>
          </div>
          {rescanScanId && (
            <div className="mb-2">
              <Scanner scanId={rescanScanId} onComplete={handleRescanComplete} />
            </div>
          )}
          <div className="flex items-center gap-3">
            <div className="flex-1">
              <SearchBar
                value={filters.search}
                onChange={handleSearchChange}
                onSubmit={searchMode === 'semantic' ? runSemanticSearch : undefined}
                placeholder={
                  searchMode === 'semantic'
                    ? "Describe the video — e.g. 'me running in the snow'"
                    : 'Search videos...'
                }
              />
            </div>
            <button
              onClick={handleToggleSemantic}
              className={`btn text-sm whitespace-nowrap ${
                searchMode === 'semantic' ? 'btn-primary' : 'btn-secondary'
              }`}
              title="Semantic (AI) search — rank videos by meaning, not just keywords"
            >
              ✨ Semantic{searchMode === 'semantic' ? ' ✓' : ''}
            </button>
          </div>
          {searchMode === 'semantic' && (
            <p className="mt-2 text-xs text-gray-500 dark:text-gray-400">
              Ranks the whole library by meaning — type a description and press{' '}
              <kbd className="px-1 bg-gray-100 dark:bg-gray-700 rounded">Enter</kbd>. Sidebar
              filters don&apos;t apply to semantic results.
            </p>
          )}
        </div>
      </header>

      {/* Main Content */}
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
        <div className="grid grid-cols-1 lg:grid-cols-4 gap-8">
          {/* Sidebar - Filters */}
          <aside className="lg:col-span-1">
            <div className="sticky top-24">
              <div className="card">
                <h2 className="text-lg font-semibold text-gray-900 dark:text-white mb-4">
                  Filters
                </h2>
                <FilterPanel
                  filters={filters}
                  categories={categories}
                  tags={tags}
                  productions={productions}
                  onFilterChange={handleFilterChange}
                  onClearFilters={handleClearFilters}
                />
              </div>
            </div>
          </aside>

          {/* Main - Video Grid */}
          <main className="lg:col-span-3">
            {/* Semantic search banners */}
            {searchMode === 'semantic' && semanticError && (
              <div className="mb-4 p-4 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
                <p className="text-sm text-red-600 dark:text-red-400">{semanticError}</p>
              </div>
            )}
            {searchMode === 'semantic' && semanticHasRun && semanticIndexEmpty && !semanticError && (
              <div className="mb-4 p-4 bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800 rounded-lg">
                <p className="text-sm text-yellow-800 dark:text-yellow-300">
                  The semantic index is empty. Build it first in{' '}
                  <button
                    onClick={() => router.push('/settings')}
                    className="underline font-medium"
                  >
                    Settings → AI / LLM → Rebuild index
                  </button>
                  , then try your search again.
                </p>
              </div>
            )}
            {searchMode === 'semantic' && semanticHasRun && semanticWeak && !semanticError && (
              <div className="mb-4 p-4 bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg">
                <p className="text-sm text-blue-800 dark:text-blue-300">
                  No strong matches — showing the closest clips. Semantic search is text-only, so
                  it can&apos;t find purely visual content that nothing describes. Try describing
                  <em> spoken</em> content, or add tags/notes/transcripts to make more videos findable.
                </p>
              </div>
            )}

            {searchMode === 'semantic' && !semanticHasRun && !isLoading ? (
              <div className="card text-center py-16">
                <p className="text-4xl mb-3">✨</p>
                <p className="text-gray-700 dark:text-gray-300 font-medium mb-1">
                  Semantic search
                </p>
                <p className="text-sm text-gray-500 dark:text-gray-400">
                  Describe what you&apos;re looking for — like{' '}
                  <em>&ldquo;me talking about parenting&rdquo;</em> — and press Enter.
                </p>
              </div>
            ) : (
              <VideoGrid
                videos={videos}
                isLoading={isLoading}
                selectedVideos={selectedVideoIds}
                onVideoSelect={handleVideoSelect}
                onVideoClick={setSelectedVideo}
              />
            )}

            {/* Pagination (keyword mode only — semantic returns a ranked top-N) */}
            {searchMode === 'keyword' && totalPages > 1 && (
              <div className="flex items-center justify-center gap-2 mt-8">
                <button
                  onClick={() => handlePageChange(pagination.page - 1)}
                  disabled={pagination.page === 1}
                  className="btn btn-secondary disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  Previous
                </button>
                <span className="text-sm text-gray-600 dark:text-gray-400 px-4">
                  Page {pagination.page} of {totalPages}
                </span>
                <button
                  onClick={() => handlePageChange(pagination.page + 1)}
                  disabled={pagination.page === totalPages}
                  className="btn btn-secondary disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  Next
                </button>
              </div>
            )}
          </main>
        </div>
      </div>

      {/* Video Modal */}
      {selectedVideo && (
        <VideoModal
          video={selectedVideo}
          isOpen={!!selectedVideo}
          onClose={() => setSelectedVideo(null)}
          onUpdate={() => {
            loadData();
            loadFilters();
          }}
        />
      )}

      {/* Production Manager Modal */}
      <ProductionManager
        isOpen={showProductionManager}
        onClose={() => setShowProductionManager(false)}
        onUpdate={() => {
          loadFilters();
        }}
      />

      {/* Bulk Actions */}
      <BulkActions
        selectedCount={selectedVideoIds.size}
        selectedVideoIds={Array.from(selectedVideoIds)}
        selectedVideos={videos.filter((v) => selectedVideoIds.has(v.id))}
        onClearSelection={handleClearSelection}
        onUpdate={() => {
          loadData();
          loadFilters();
          handleClearSelection();
        }}
      />
    </div>
  );
}
