import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { PhotoSlider } from 'react-photo-view'
import 'react-photo-view/dist/react-photo-view.css'
import { defaultRangeExtractor, useVirtualizer, type Range } from '@tanstack/react-virtual'
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
const MIN_TILE = 136 // ~132px tile + gap → used to derive the column count

type Order = 'newest' | 'oldest'

type Cell = { photo: Photo; index: number } // index = flat index into `ordered`

interface Group {
  key: string
  label: string
  items: Cell[]
}

// A virtualized row: a date header, or one grid line of up to `cols` tiles.
type Row =
  | { type: 'header'; key: string; label: string; items: Cell[] }
  | { type: 'tiles'; key: string; cells: Cell[] }

function formatDay(key: string): string {
  if (key === 'unknown') return 'Unknown date'
  const d = new Date(key + 'T00:00:00')
  if (isNaN(d.getTime())) return key
  return d.toLocaleDateString(undefined, { year: 'numeric', month: 'long', day: 'numeric' })
}

function groupByDate(photos: Photo[]): Group[] {
  const map = new Map<string, Cell[]>()
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

// Flatten the ordered photos into virtual rows. Grouped → header + tile rows per
// day; flat → tile rows only. Each cell carries its flat index into `ordered`.
function buildRows(ordered: Photo[], grouped: boolean, cols: number): Row[] {
  const rows: Row[] = []
  const pushTiles = (cells: Cell[], keyBase: string) => {
    for (let i = 0; i < cells.length; i += cols) {
      const line = cells.slice(i, i + cols)
      rows.push({ type: 'tiles', key: `t:${keyBase}:${line[0].photo.id}`, cells: line })
    }
  }
  if (grouped) {
    for (const g of groupByDate(ordered)) {
      rows.push({ type: 'header', key: `h:${g.key}`, label: g.label, items: g.items })
      pushTiles(g.items, g.key)
    }
  } else {
    pushTiles(
      ordered.map((photo, index) => ({ photo, index })),
      'all',
    )
  }
  return rows
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

  const loadedRef = useRef(0)
  const totalRef = useRef<number | null>(null)
  const genRef = useRef(0)
  const loadingRef = useRef(false)
  const doneRef = useRef(false)

  // --- Paging core (data side; unchanged by virtualization) --------------------
  const loadMore = useCallback(async () => {
    if (loadingRef.current || doneRef.current) return
    const gen = genRef.current
    loadingRef.current = true
    setLoading(true)
    setError(null)
    try {
      if (order === 'newest' && totalRef.current === null) {
        const probe = await fetchPage(0, 1)
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
        chunk = page.photos.slice().reverse()
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

  // First load, and reset + reload from the right end when the order changes.
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

  // Display order: sort the loaded photos by date (idempotent in the common case,
  // but keeps display correct if the camera's own order ever differs).
  const ordered = useMemo(() => {
    const asc = [...photos].sort((a, b) => {
      const da = a.date || ''
      const db = b.date || ''
      return da < db ? -1 : da > db ? 1 : 0
    })
    return order === 'oldest' ? asc : asc.reverse()
  }, [photos, order])

  // --- Selection (operates on `ordered`; unchanged) ----------------------------
  useEffect(() => setAnchor(null), [order, grouped])

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

  const toggleGroup = useCallback((items: Cell[]) => {
    setSelected((prev) => {
      const next = new Set(prev)
      const all = items.every((it) => next.has(it.photo.id))
      for (const it of items) {
        if (all) next.delete(it.photo.id)
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
    const picked = ordered.filter((p) => selected.has(p.id))
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
  }, [ordered, selected])

  // --- Virtualized rows --------------------------------------------------------
  const parentRef = useRef<HTMLDivElement | null>(null)
  const [cols, setCols] = useState(2)

  // Responsive column count from the scroll container width.
  useEffect(() => {
    const el = parentRef.current
    if (!el) return
    const compute = () => setCols(Math.max(1, Math.floor((el.clientWidth - 24) / MIN_TILE)))
    compute()
    const ro = new ResizeObserver(compute)
    ro.observe(el)
    return () => ro.disconnect()
  }, [])

  const rows = useMemo(() => buildRows(ordered, grouped, cols), [ordered, grouped, cols])

  const stickyIndexes = useMemo(() => {
    const arr: number[] = []
    rows.forEach((r, i) => {
      if (r.type === 'header') arr.push(i)
    })
    return arr
  }, [rows])
  const activeStickyRef = useRef(0)

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => parentRef.current,
    estimateSize: (i) => (rows[i].type === 'header' ? 40 : 168),
    overscan: 6,
    getItemKey: (i) => rows[i].key,
    rangeExtractor: useCallback(
      (range: Range) => {
        let active = 0
        for (const i of stickyIndexes) {
          if (i <= range.startIndex) active = i
          else break
        }
        activeStickyRef.current = active
        const set = new Set<number>([active, ...defaultRangeExtractor(range)])
        return [...set].sort((a, b) => a - b)
      },
      [stickyIndexes],
    ),
  })

  const virtualItems = virtualizer.getVirtualItems()

  // Re-measure when the row model's shape changes; scroll to top on toggles.
  useEffect(() => {
    virtualizer.measure()
  }, [grouped, order, cols, virtualizer])
  useEffect(() => {
    parentRef.current?.scrollTo({ top: 0 })
  }, [grouped, order])

  // Infinite load driven by the virtual range: fetch when the last rendered row
  // nears the end of the rows (auto-fills short pages too).
  useEffect(() => {
    if (!rows.length) return
    const last = virtualItems[virtualItems.length - 1]
    if (last && last.index >= rows.length - 2) loadMore()
  }, [virtualItems, rows.length, loadMore])

  // --- Lightbox (controlled PhotoSlider over the full loaded list) -------------
  const [viewerIndex, setViewerIndex] = useState(0)
  const [viewerVisible, setViewerVisible] = useState(false)
  const sliderImages = useMemo(
    () => ordered.map((p) => ({ src: p.fullUrl, key: p.id })),
    [ordered],
  )
  const openViewer = useCallback((index: number) => {
    setViewerIndex(index)
    setViewerVisible(true)
  }, [])
  // Swiping to the last loaded photo loads the next page so it never dead-ends.
  useEffect(() => {
    if (viewerVisible && viewerIndex >= ordered.length - 2) loadMore()
  }, [viewerVisible, viewerIndex, ordered.length, loadMore])

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

      <div className="scroller" ref={parentRef}>
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

        {loaded > 0 && (
          <div className="v-list" style={{ height: virtualizer.getTotalSize() }}>
            {virtualItems.map((vi) => {
              const row = rows[vi.index]
              if (!row) return null
              const isHeader = row.type === 'header'
              const sticky = isHeader && activeStickyRef.current === vi.index
              return (
                <div
                  key={vi.key}
                  ref={virtualizer.measureElement}
                  data-index={vi.index}
                  className="v-row"
                  style={
                    sticky
                      ? { position: 'sticky', top: 0, zIndex: 12 }
                      : { position: 'absolute', top: 0, transform: `translateY(${vi.start}px)` }
                  }
                >
                  {row.type === 'header' ? (
                    <HeaderRow row={row} selected={selected} onToggleGroup={toggleGroup} />
                  ) : (
                    <div
                      className="tiles-row"
                      style={{ gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))` }}
                    >
                      {row.cells.map(({ photo, index }) => (
                        <Tile
                          key={photo.id}
                          photo={photo}
                          index={index}
                          selected={selected.has(photo.id)}
                          onToggle={toggle}
                          onOpen={openViewer}
                        />
                      ))}
                    </div>
                  )}
                </div>
              )
            })}
          </div>
        )}

        {loaded > 0 && loading && (
          <div className="flex items-center justify-center gap-2 py-6 text-sm text-muted-foreground">
            <Loader2 className="h-4 w-4 animate-spin" /> Loading more…
          </div>
        )}
        {loaded > 0 && error && (
          <div className="flex items-center justify-center gap-3 py-6 text-sm">
            <span className="text-destructive">{error}</span>
            <Button size="sm" onClick={() => loadMore()}>
              Retry
            </Button>
          </div>
        )}
        {loaded > 0 && !hasMore && (
          <div className="py-6 text-center text-xs text-muted-foreground">
            All {loaded} photos loaded
          </div>
        )}
      </div>

      <PhotoSlider
        images={sliderImages}
        visible={viewerVisible}
        onClose={() => setViewerVisible(false)}
        index={viewerIndex}
        onIndexChange={setViewerIndex}
      />
    </div>
  )
}

function HeaderRow({
  row,
  selected,
  onToggleGroup,
}: {
  row: { label: string; items: Cell[] }
  selected: Set<string>
  onToggleGroup: (items: Cell[]) => void
}) {
  const allSel = row.items.every((it) => selected.has(it.photo.id))
  const someSel = !allSel && row.items.some((it) => selected.has(it.photo.id))
  return (
    <div className="date-h" onClick={() => onToggleGroup(row.items)} role="button" tabIndex={0}>
      <span className={'gcheck' + (allSel ? ' on' : someSel ? ' some' : '')} />
      <span className="date-label">{row.label}</span>
      <span className="muted">{row.items.length}</span>
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

// A tile: click the image to preview (lightbox), click the checkbox or filename
// to select. Selection shows as a highlighted filename (macOS Finder style).
function Tile({
  photo,
  index,
  selected,
  onToggle,
  onOpen,
}: {
  photo: Photo
  index: number
  selected: boolean
  onToggle: (index: number, shift: boolean) => void
  onOpen: (index: number) => void
}) {
  return (
    <figure className={selected ? 'tile selected' : 'tile'}>
      <div className="thumb">
        <img
          src={photo.thumbUrl}
          alt={photo.name}
          loading="lazy"
          decoding="async"
          draggable={false}
          className="cursor-zoom-in"
          onClick={() => onOpen(index)}
          onLoad={(e) => e.currentTarget.classList.add('loaded')}
        />
        <Checkbox
          checked={selected}
          onCheckedChange={() => {}}
          onClick={(e) => {
            e.stopPropagation()
            onToggle(index, e.shiftKey)
          }}
          className="check"
        />
      </div>
      <figcaption title={photo.name} onClick={(e) => onToggle(index, e.shiftKey)}>
        {photo.name}
      </figcaption>
    </figure>
  )
}
