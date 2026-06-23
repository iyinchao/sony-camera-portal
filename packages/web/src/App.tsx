import { useCallback, useEffect, useRef, useState } from 'react'
import { connectCamera, getState, type ConnState } from './api'
import ConnectPanel from './ConnectPanel'
import Gallery from './Gallery'
import './App.css'

type View = 'connecting' | 'connect' | 'gallery'

export default function App() {
  const [conn, setConn] = useState<ConnState | null>(null)
  const [view, setView] = useState<View>('connecting')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  // Apply a /api/connect or /api/state result: on success show the gallery
  // (which loads its own pages), otherwise show the connect panel with the error.
  const applyState = useCallback((st: ConnState) => {
    setConn(st)
    if (st.connected && !st.error) {
      setError(null)
      setView('gallery')
    } else {
      setError(st.error || 'No camera found')
      setView('connect')
    }
  }, [])

  const autoConnect = useCallback(async () => {
    setBusy(true)
    setError(null)
    setView('connecting')
    applyState(await connectCamera())
    setBusy(false)
  }, [applyState])

  const manualConnect = useCallback(
    async (host: string) => {
      setBusy(true)
      setError(null)
      applyState(await connectCamera(host))
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
          applyState(st)
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
    // Key by host so switching cameras remounts the gallery with fresh paging.
    return (
      <Gallery
        key={conn?.host ?? 'camera'}
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
