import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { PhotoProvider, PhotoView } from 'react-photo-view'
import 'react-photo-view/dist/react-photo-view.css'
import {
  ArrowDownWideNarrow,
  ArrowUpWideNarrow,
  CalendarDays,
  LayoutGrid,
  Loader2,
} from 'lucide-react'
import { fetchPage, type Photo } from './api'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'

const PAGE_LIMIT = 60

type Order = 'newest' | 'oldest'

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
  const [order, setOrder] = useState<Order>('newest')
  const [grouped, setGrouped] = useState(true)

  const loadedRef = useRef(0) // how many photos loaded so far
  const totalRef = useRef<number | null>(null)
  const genRef = useRef(0) // load-session token; bumping it discards in-flight loads
  const loadingRef = useRef(false)
  const doneRef = useRef(false)
  const sentinelRef = useRef<HTMLDivElement | null>(null)

  // Fetch the next chunk in the current sort direction and append.
  // 'oldest' pages forward from offset 0. 'newest' pages backward from the end
  // (using the total) so the camera's newest photos appear first and the list
  // doesn't reflow as more loads. If the total is unknown we fall back to a
  // forward load and let the client-side `ordered` sort handle display.
  // A generation token (genRef) discards results from a superseded session
  // (order change / StrictMode double-mount); guards against concurrent calls.
  const loadMore = useCallback(async () => {
    if (loadingRef.current || doneRef.current) return
    const gen = genRef.current
    loadingRef.current = true
    setLoading(true)
    setError(null)
    try {
      if (order === 'newest' && totalRef.current === null) {
        const probe = await fetchPage(0, 1) // learn the total so we can page from the end
        if (gen !== genRef.current) return
        if (probe.total !== null) {
          totalRef.current = probe.total
          setTotal(probe.total)
        }
      }

      let chunk: Photo[]
      let atEnd: boolean
      const tot = totalRef.current

      if (order === 'newest' && tot !== null) {
        const remaining = tot - loadedRef.current
        if (remaining <= 0) {
          doneRef.current = true
          setHasMore(false)
          return
        }
        const start = Math.max(0, remaining - PAGE_LIMIT)
        const page = await fetchPage(start, remaining - start)
        if (gen !== genRef.current) return
        chunk = page.photos.slice().reverse() // newest-first within the chunk
        atEnd = start === 0 || page.photos.length === 0
      } else {
        const page = await fetchPage(loadedRef.current, PAGE_LIMIT)
        if (gen !== genRef.current) return
        if (page.total !== null) {
          totalRef.current = page.total
          setTotal(page.total)
        }
        chunk = page.photos
        atEnd = !page.hasMore || page.photos.length === 0
      }

      loadedRef.current += chunk.length
      if (atEnd) doneRef.current = true
      setPhotos((prev) => {
        const seen = new Set(prev.map((p) => p.id))
        return [...prev, ...chunk.filter((p) => !seen.has(p.id))]
      })
      setHasMore(!atEnd)
    } catch (e) {
      if (gen === genRef.current) setError(e instanceof Error ? e.message : String(e))
    } finally {
      if (gen === genRef.current) {
        loadingRef.current = false
        setLoading(false)
      }
    }
  }, [order])

  // Load the first chunk, and reset + reload from the right end when the sort
  // direction changes. Bumping genRef invalidates any in-flight load.
  useEffect(() => {
    genRef.current += 1
    loadingRef.current = false
    doneRef.current = false
    loadedRef.current = 0
    setPhotos([])
    setHasMore(true)
    setError(null)
    setAnchor(null)
    loadMore()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [order])

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

  // Display order: sort the loaded photos by date, newest or oldest first.
  // (Sorting client-side keeps it correct regardless of the camera's own order.)
  const ordered = useMemo(() => {
    const asc = [...photos].sort((a, b) => {
      const da = a.date || ''
      const db = b.date || ''
      return da < db ? -1 : da > db ? 1 : 0
    })
    return order === 'oldest' ? asc : asc.reverse()
  }, [photos, order])

  const groups = useMemo(() => (grouped ? groupByDate(ordered) : []), [ordered, grouped])

  // Changing order/grouping invalidates the shift-range anchor (indices moved).
  useEffect(() => setAnchor(null), [order, grouped])

  // Plain click toggles one; shift-click selects the range from the last anchor.
  const toggle = useCallback(
    (index: number, shift: boolean) => {
      setSelected((prev) => {
        const next = new Set(prev)
        if (shift && anchor !== null) {
          const lo = Math.min(anchor, index)
          const hi = Math.max(anchor, index)
          const selecting = !next.has(ordered[index].id)
          for (let i = lo; i <= hi; i++) {
            if (selecting) next.add(ordered[i].id)
            else next.delete(ordered[i].id)
          }
        } else {
          const id = ordered[index].id
          if (next.has(id)) next.delete(id)
          else next.add(id)
        }
        return next
      })
      setAnchor(index)
    },
    [anchor, ordered],
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
        <Button
          variant="ghost"
          size="sm"
          onClick={() => setOrder((o) => (o === 'newest' ? 'oldest' : 'newest'))}
          disabled={loaded === 0}
          title={order === 'newest' ? 'Newest first' : 'Oldest first'}
        >
          {order === 'newest' ? (
            <ArrowDownWideNarrow className="h-4 w-4" />
          ) : (
            <ArrowUpWideNarrow className="h-4 w-4" />
          )}
          {order === 'newest' ? 'Newest' : 'Oldest'}
        </Button>
        <Button
          variant={grouped ? 'outline' : 'ghost'}
          size="sm"
          onClick={() => setGrouped((g) => !g)}
          disabled={loaded === 0}
          title={grouped ? 'Grouped by date' : 'Ungrouped'}
        >
          {grouped ? <CalendarDays className="h-4 w-4" /> : <LayoutGrid className="h-4 w-4" />}
          {grouped ? 'By date' : 'All'}
        </Button>
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
        {grouped ? (
          groups.map((g) => {
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
          })
        ) : (
          <div className="grid">
            {ordered.map((photo, index) => (
              <Tile
                key={photo.id}
                photo={photo}
                selected={selected.has(photo.id)}
                onToggle={(shift) => toggle(index, shift)}
              />
            ))}
          </div>
        )}
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
