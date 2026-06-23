import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { PhotoProvider, PhotoView } from 'react-photo-view'
import 'react-photo-view/dist/react-photo-view.css'
import { Loader2 } from 'lucide-react'
import { fetchPage, type Photo } from './api'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'

const PAGE_LIMIT = 60

interface Group {
  key: string
  label: string
  items: { photo: Photo; index: number }[] // index = global flat index (for shift-range)
}

function formatDay(key: string): string {
  if (key === 'unknown') return 'Unknown date'
  const d = new Date(key + 'T00:00:00')
  if (isNaN(d.getTime())) return key
  return d.toLocaleDateString(undefined, { year: 'numeric', month: 'long', day: 'numeric' })
}

function groupByDate(photos: Photo[]): Group[] {
  const map = new Map<string, { photo: Photo; index: number }[]>()
  const order: string[] = []
  photos.forEach((photo, index) => {
    const key = (photo.date || '').slice(0, 10) || 'unknown'
    if (!map.has(key)) {
      map.set(key, [])
      order.push(key)
    }
    map.get(key)!.push({ photo, index })
  })
  return order.map((key) => ({ key, label: formatDay(key), items: map.get(key)! }))
}

export default function Gallery({
  host,
  onChangeCamera,
}: {
  host: string | null
  onChangeCamera: () => void
}) {
  const [photos, setPhotos] = useState<Photo[]>([])
  const [total, setTotal] = useState<number | null>(null)
  const [hasMore, setHasMore] = useState(true)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [anchor, setAnchor] = useState<number | null>(null)

  const offsetRef = useRef(0)
  const loadingRef = useRef(false)
  const doneRef = useRef(false)
  const sentinelRef = useRef<HTMLDivElement | null>(null)

  // Fetch the next page and append. Guards against concurrent/after-end calls.
  const loadMore = useCallback(async () => {
    if (loadingRef.current || doneRef.current) return
    loadingRef.current = true
    setLoading(true)
    setError(null)
    try {
      const page = await fetchPage(offsetRef.current, PAGE_LIMIT)
      offsetRef.current += page.photos.length
      if (!page.hasMore || page.photos.length === 0) doneRef.current = true
      setPhotos((prev) => [...prev, ...page.photos])
      setTotal(page.total)
      setHasMore(page.hasMore && page.photos.length > 0)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      loadingRef.current = false
      setLoading(false)
    }
  }, [])

  // First page on mount.
  useEffect(() => {
    loadMore()
  }, [loadMore])

  // Load more as the bottom sentinel approaches (also auto-fills short pages).
  useEffect(() => {
    const el = sentinelRef.current
    if (!el) return
    const io = new IntersectionObserver(
      (entries) => {
        if (entries[0].isIntersecting) loadMore()
      },
      { rootMargin: '800px' },
    )
    io.observe(el)
    return () => io.disconnect()
  }, [loadMore])

  const groups = useMemo(() => groupByDate(photos), [photos])

  // Plain click toggles one; shift-click selects the range from the last anchor.
  const toggle = useCallback(
    (index: number, shift: boolean) => {
      setSelected((prev) => {
        const next = new Set(prev)
        if (shift && anchor !== null) {
          const lo = Math.min(anchor, index)
          const hi = Math.max(anchor, index)
          const selecting = !next.has(photos[index].id)
          for (let i = lo; i <= hi; i++) {
            if (selecting) next.add(photos[i].id)
            else next.delete(photos[i].id)
          }
        } else {
          const id = photos[index].id
          if (next.has(id)) next.delete(id)
          else next.add(id)
        }
        return next
      })
      setAnchor(index)
    },
    [anchor, photos],
  )

  const toggleGroup = useCallback((g: Group) => {
    setSelected((prev) => {
      const next = new Set(prev)
      const allSelected = g.items.every((it) => next.has(it.photo.id))
      for (const it of g.items) {
        if (allSelected) next.delete(it.photo.id)
        else next.add(it.photo.id)
      }
      return next
    })
  }, [])

  const selectAll = useCallback(() => setSelected(new Set(photos.map((p) => p.id))), [photos])
  const clearSelection = useCallback(() => {
    setSelected(new Set())
    setAnchor(null)
  }, [])

  const downloadSelected = useCallback(() => {
    const picked = photos.filter((p) => selected.has(p.id))
    picked.forEach((p, i) => {
      window.setTimeout(() => {
        const a = document.createElement('a')
        a.href = p.fullUrl
        a.download = p.name
        document.body.appendChild(a)
        a.click()
        a.remove()
      }, i * 300)
    })
  }, [photos, selected])

  const count = selected.size
  const loaded = photos.length

  return (
    <div className="app">
      <header className="toolbar">
        <div className="brand">
          <span className="dot" /> Sony Camera Portal
        </div>
        <button className="host-chip" onClick={onChangeCamera} title="Change camera">
          <span className="host-dot" /> {host ?? 'camera'} <span className="host-change">change</span>
        </button>
        <div className="spacer" />
        <span className="muted count">
          {loaded}
          {total != null ? ` / ${total}` : ''} photos{count > 0 ? ` · ${count} selected` : ''}
        </span>
        <Button variant="ghost" size="sm" onClick={selectAll} disabled={loaded === 0}>
          Select all
        </Button>
        <Button variant="ghost" size="sm" onClick={clearSelection} disabled={count === 0}>
          Clear
        </Button>
        <Button variant="primary" size="sm" onClick={downloadSelected} disabled={count === 0}>
          Download{count > 0 ? ` (${count})` : ''}
        </Button>
      </header>

      {loaded === 0 && loading && <SkeletonGrid />}
      {loaded === 0 && !loading && error && (
        <div className="centered">
          <p className="error-title">Couldn’t load photos</p>
          <p className="muted">{error}</p>
          <Button onClick={() => loadMore()}>Retry</Button>
        </div>
      )}
      {loaded === 0 && !loading && !error && !hasMore && (
        <div className="centered">No photos found on the camera.</div>
      )}

      <PhotoProvider>
        {groups.map((g) => {
          const allSel = g.items.every((it) => selected.has(it.photo.id))
          const someSel = !allSel && g.items.some((it) => selected.has(it.photo.id))
          return (
            <section key={g.key} className="group">
              <div className="date-h" onClick={() => toggleGroup(g)} role="button" tabIndex={0}>
                <span className={'gcheck' + (allSel ? ' on' : someSel ? ' some' : '')} />
                <span className="date-label">{g.label}</span>
                <span className="muted">{g.items.length}</span>
              </div>
              <div className="grid">
                {g.items.map(({ photo, index }) => (
                  <Tile
                    key={photo.id}
                    photo={photo}
                    selected={selected.has(photo.id)}
                    onToggle={(shift) => toggle(index, shift)}
                  />
                ))}
              </div>
            </section>
          )
        })}
      </PhotoProvider>

      {/* Infinite-scroll sentinel + status */}
      <div ref={sentinelRef} aria-hidden className="h-px w-full" />
      {loading && loaded > 0 && (
        <div className="flex items-center justify-center gap-2 py-6 text-sm text-muted-foreground">
          <Loader2 className="h-4 w-4 animate-spin" /> Loading more…
        </div>
      )}
      {error && loaded > 0 && (
        <div className="flex items-center justify-center gap-3 py-6 text-sm">
          <span className="text-destructive">{error}</span>
          <Button size="sm" onClick={() => loadMore()}>
            Retry
          </Button>
        </div>
      )}
      {!hasMore && loaded > 0 && (
        <div className="py-6 text-center text-xs text-muted-foreground">
          All {loaded} photos loaded
        </div>
      )}
    </div>
  )
}

function SkeletonGrid() {
  return (
    <div className="grid">
      {Array.from({ length: 18 }).map((_, i) => (
        <div key={i} className="tile skel" />
      ))}
    </div>
  )
}

// A tile: click the image to preview (lightbox), click the checkbox to select.
// The two actions are kept separate so they don't conflict.
function Tile({
  photo,
  selected,
  onToggle,
}: {
  photo: Photo
  selected: boolean
  onToggle: (shift: boolean) => void
}) {
  return (
    <figure className={selected ? 'tile selected' : 'tile'}>
      <PhotoView src={photo.fullUrl}>
        <img
          src={photo.thumbUrl}
          alt={photo.name}
          loading="lazy"
          decoding="async"
          draggable={false}
          className="cursor-zoom-in"
          onLoad={(e) => e.currentTarget.classList.add('loaded')}
        />
      </PhotoView>
      <Checkbox
        checked={selected}
        onCheckedChange={() => {}}
        onClick={(e) => {
          e.stopPropagation()
          onToggle(e.shiftKey)
        }}
        className="absolute left-2 top-2 cursor-pointer"
      />
      <figcaption title={photo.name}>{photo.name}</figcaption>
    </figure>
  )
}
