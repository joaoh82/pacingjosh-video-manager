# Video Manager Frontend

Next.js frontend for the Video Manager application.

## Setup

### 1. Install Dependencies

```bash
npm install
# or
yarn install
# or
pnpm install
```

### 2. Configure Environment

Copy `.env.example` to `.env.local`:

```bash
cp .env.example .env.local
```

Edit `.env.local` to configure the API URL (default: http://localhost:8000/api).

### 3. Run Development Server

```bash
npm run dev
# or
yarn dev
# or
pnpm dev
```

Open [http://localhost:3000](http://localhost:3000) in your browser.

## Project Structure

```
frontend/
├── src/
│   ├── app/              # Next.js App Router pages
│   │   ├── layout.tsx    # Root layout
│   │   ├── page.tsx      # Main video browser page
│   │   └── setup/        # Initial setup page
│   ├── components/       # React components
│   │   ├── VideoGrid.tsx
│   │   ├── VideoCard.tsx
│   │   ├── VideoModal.tsx
│   │   ├── SearchBar.tsx
│   │   ├── FilterPanel.tsx
│   │   └── ...
│   ├── lib/              # Utilities and API client
│   │   ├── api.ts        # API client functions
│   │   └── types.ts      # TypeScript types
│   └── styles/
│       └── globals.css   # Global styles with Tailwind
└── public/               # Static assets
```

## Features

- 🎬 Video browsing with thumbnail previews
- 🔍 Full-text search and filtering
- 🏷️ Tag and category management
- 📊 Video statistics dashboard
- 🎨 Responsive design with Tailwind CSS
- 🌙 Dark mode support
- ⚡ Fast and optimized with Next.js 14

## Scripts

- `npm run dev` - Start development server
- `npm run build` - Build for production
- `npm run start` - Start production server
- `npm run lint` - Run ESLint

## Configuration

The application connects to the backend API at `http://localhost:8000` by default.

To change the API URL, update `NEXT_PUBLIC_API_URL` in `.env.local`.

## Technologies

- **Framework**: Next.js 14 (App Router)
- **UI**: React 18
- **Styling**: Tailwind CSS
- **Language**: TypeScript
- **Data Fetching**: React Query (TanStack Query)
- **Date Handling**: date-fns

## Development

### Adding a New Component

1. Create component file in `src/components/`
2. Use TypeScript for type safety
3. Follow existing component patterns
4. Add proper prop types and documentation

### API Integration

All API calls are centralized in `src/lib/api.ts`. To add a new endpoint:

1. Add types in `src/lib/types.ts`
2. Add API function in `src/lib/api.ts`
3. Use the function in components with React Query

## Next Steps

Phase 4 ✅ Complete:
- [x] Project structure
- [x] TypeScript configuration
- [x] Tailwind CSS setup
- [x] API client
- [x] Type definitions
- [x] Root layout

Phase 5: Frontend Components (Coming next)
