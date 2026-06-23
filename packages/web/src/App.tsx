import { useCallback, useEffect, useMemo, useState, type ReactNode } from 'react'
import { fetchPhotos, type Photo } from './api'
import './App.css'

type Status =
  | { kind: 'loading' }
  | { kind: 'error'; message: string }
  | { kind: 'ready' }

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

export default function App() {
  const [photos, setPhotos] = useState<Photo[]>([])
  const [status, setStatus] = useState<Status>({ kind: 'loading' })
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [anchor, setAnchor] = useState<number | null>(null)

  const load = useCallback((signal?: AbortSignal) => {
    setStatus({ kind: 'loading' })
    fetchPhotos(signal)
      .then((p) => {
        setPhotos(p)
        setStatus({ kind: 'ready' })
      })
      .catch((e: unknown) => {
        if (signal?.aborted) return
        setStatus({ kind: 'error', message: e instanceof Error ? e.message : String(e) })
      })
  }, [])

  useEffect(() => {
    const ctrl = new AbortController()
    load(ctrl.signal)
    return () => ctrl.abort()
  }, [load])

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

  return (
    <div className="app">
      <header className="toolbar">
        <div className="brand">
          <span className="dot" /> Sony Camera Portal
        </div>
        <div className="spacer" />
        {status.kind === 'ready' && (
          <span className="muted count">
            {photos.length} photos{count > 0 ? ` · ${count} selected` : ''}
          </span>
        )}
        <button onClick={selectAll} disabled={status.kind !== 'ready' || photos.length === 0}>
          Select all
        </button>
        <button onClick={clearSelection} disabled={count === 0}>
          Clear
        </button>
        <button className="primary" onClick={downloadSelected} disabled={count === 0}>
          Download{count > 0 ? ` (${count})` : ''}
        </button>
      </header>

      {status.kind === 'loading' && <SkeletonGrid />}

      {status.kind === 'error' && (
        <Centered>
          <p className="error-title">Couldn’t load photos</p>
          <p className="muted">{status.message}</p>
          <p className="muted">
            Make sure you’re connected to the camera’s Wi-Fi (Select on Smartphone).
          </p>
          <button onClick={() => load()}>Retry</button>
        </Centered>
      )}

      {status.kind === 'ready' && photos.length === 0 && (
        <Centered>No photos found on the camera.</Centered>
      )}

      {status.kind === 'ready' &&
        groups.map((g) => {
          const allSel = g.items.every((it) => selected.has(it.photo.id))
          const someSel = !allSel && g.items.some((it) => selected.has(it.photo.id))
          return (
            <section key={g.key} className="group">
              <div
                className="date-h"
                onClick={() => toggleGroup(g)}
                role="button"
                tabIndex={0}
              >
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
                    onClick={(e) => toggle(index, e.shiftKey)}
                  />
                ))}
              </div>
            </section>
          )
        })}
    </div>
  )
}

function Tile({
  photo,
  selected,
  onClick,
}: {
  photo: Photo
  selected: boolean
  onClick: (e: React.MouseEvent) => void
}) {
  return (
    <figure className={selected ? 'tile selected' : 'tile'} onClick={onClick}>
      <img
        src={photo.thumbUrl}
        alt={photo.name}
        loading="lazy"
        decoding="async"
        draggable={false}
        onLoad={(e) => e.currentTarget.classList.add('loaded')}
      />
      <input type="checkbox" className="check" checked={selected} readOnly tabIndex={-1} />
      <figcaption title={photo.name}>{photo.name}</figcaption>
    </figure>
  )
}

function SkeletonGrid() {
  return (
    <div className="grid skel-grid">
      {Array.from({ length: 18 }).map((_, i) => (
        <div key={i} className="tile skel" />
      ))}
    </div>
  )
}

function Centered({ children }: { children: ReactNode }) {
  return <div className="centered">{children}</div>
}
