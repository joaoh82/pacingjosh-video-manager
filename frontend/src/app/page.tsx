'use client';

import { useState, useEffect } from 'react';
import { useRouter } from 'next/navigation';
import SearchBar from '@/components/SearchBar';
import FilterPanel from '@/components/FilterPanel';
import VideoGrid from '@/components/VideoGrid';
import VideoModal from '@/components/VideoModal';
import BulkActions from '@/components/BulkActions';
import Scanner from '@/components/Scanner';
import { FilterState, Video } from '@/lib/types';
import { getConfig, getVideos, getCategories, getTags, getStatistics, rescanDirectory } from '@/lib/api';

export default function HomePage() {
  const router = useRouter();
  const [isLoading, setIsLoading] = useState(true);
  const [isConfigured, setIsConfigured] = useState(false);
  const [videos, setVideos] = useState<Video[]>([]);
  const [totalVideos, setTotalVideos] = useState(0);
  const [totalPages, setTotalPages] = useState(0);
  const [categories, setCategories] = useState<any[]>([]);
  const [tags, setTags] = useState<any[]>([]);
  const [selectedVideo, setSelectedVideo] = useState<Video | null>(null);
  const [selectedVideoIds, setSelectedVideoIds] = useState<Set<number>>(new Set());
  const [rescanScanId, setRescanScanId] = useState<string | null>(null);

  const [filters, setFilters] = useState<FilterState>({
    search: '',
    category: '',
    tags: [],
    dateFrom: null,
    dateTo: null,
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

  // Load data when filters or pagination change
  useEffect(() => {
    if (isConfigured) {
      loadData();
    }
  }, [filters, pagination, isConfigured]);

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
      const [categoriesData, tagsData] = await Promise.all([
        getCategories(),
        getTags(),
      ]);
      setCategories(categoriesData);
      setTags(tagsData);
    } catch (error) {
      console.error('Failed to load filters:', error);
    }
  };

  const handleFilterChange = (newFilters: Partial<FilterState>) => {
    setFilters({ ...filters, ...newFilters });
    setPagination({ ...pagination, page: 1 }); // Reset to first page
  };

  const handleClearFilters = () => {
    setFilters({
      search: '',
      category: '',
      tags: [],
      dateFrom: null,
      dateTo: null,
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
            <h1 className="text-2xl font-bold text-gray-900 dark:text-white">
              Video Manager
            </h1>
            <div className="flex items-center gap-4">
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
                ⚙️ Settings
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
          <SearchBar
            value={filters.search}
            onChange={(search) => handleFilterChange({ search })}
          />
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
                  onFilterChange={handleFilterChange}
                  onClearFilters={handleClearFilters}
                />
              </div>
            </div>
          </aside>

          {/* Main - Video Grid */}
          <main className="lg:col-span-3">
            <VideoGrid
              videos={videos}
              isLoading={isLoading}
              selectedVideos={selectedVideoIds}
              onVideoSelect={handleVideoSelect}
              onVideoClick={setSelectedVideo}
            />

            {/* Pagination */}
            {totalPages > 1 && (
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

      {/* Bulk Actions */}
      <BulkActions
        selectedCount={selectedVideoIds.size}
        selectedVideoIds={Array.from(selectedVideoIds)}
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
