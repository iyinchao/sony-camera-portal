import { useCallback, useEffect, useRef, useState } from 'react'
import { connectCamera, fetchPhotos, getState, type ConnState, type Photo } from './api'
import ConnectPanel from './ConnectPanel'
import Gallery from './Gallery'
import './App.css'

type View = 'connecting' | 'connect' | 'gallery'

export default function App() {
  const [conn, setConn] = useState<ConnState | null>(null)
  const [view, setView] = useState<View>('connecting')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [photos, setPhotos] = useState<Photo[]>([])

  const loadGallery = useCallback(async () => {
    const ps = await fetchPhotos()
    setPhotos(ps)
    setView('gallery')
  }, [])

  // Apply a /api/connect or /api/state result: on success load the gallery,
  // otherwise show the connect panel with the error.
  const applyState = useCallback(
    async (st: ConnState) => {
      setConn(st)
      if (st.connected && !st.error) {
        try {
          await loadGallery()
          setError(null)
          return
        } catch (e) {
          setError(e instanceof Error ? e.message : String(e))
        }
      } else {
        setError(st.error || 'No camera found')
      }
      setView('connect')
    },
    [loadGallery],
  )

  const autoConnect = useCallback(async () => {
    setBusy(true)
    setError(null)
    setView('connecting')
    await applyState(await connectCamera())
    setBusy(false)
  }, [applyState])

  const manualConnect = useCallback(
    async (host: string) => {
      setBusy(true)
      setError(null)
      await applyState(await connectCamera(host))
      setBusy(false)
    },
    [applyState],
  )

  // Bootstrap once: if already connected show the gallery, else auto-discover.
  const started = useRef(false)
  useEffect(() => {
    if (started.current) return
    started.current = true
    ;(async () => {
      try {
        const st = await getState()
        if (st.connected) {
          await applyState(st)
        } else {
          await autoConnect()
        }
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e))
        setView('connect')
      }
    })()
  }, [applyState, autoConnect])

  if (view === 'gallery') {
    return (
      <Gallery
        photos={photos}
        host={conn?.host ?? null}
        onChangeCamera={() => setView('connect')}
      />
    )
  }

  return (
    <ConnectPanel
      busy={busy || view === 'connecting'}
      error={error}
      onAuto={autoConnect}
      onManual={manualConnect}
      onCancel={conn?.connected ? () => setView('gallery') : undefined}
    />
  )
}
