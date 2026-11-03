import { useEffect, useState, type ReactNode } from 'react'

const DURATION_MS = 400

export function BottomSheet({
  open,
  onClose,
  children,
}: {
  open: boolean
  onClose: () => void
  children: ReactNode
}) {
  const [mounted, setMounted] = useState(false)
  const [closing, setClosing] = useState(false)

  useEffect(() => {
    if (open) {
      setClosing(false)
      setMounted(true)
      return
    }
    if (!mounted) return
    setClosing(true)
    const id = setTimeout(() => {
      setMounted(false)
      setClosing(false)
    }, DURATION_MS)
    return () => clearTimeout(id)
  }, [open])

  if (!mounted) return null

  return (
    <div className="fixed inset-0 z-50">
      <button
        type="button"
        aria-label="Close"
        onClick={onClose}
        className={`absolute inset-0 bg-black/40 ${closing ? 'animate-sheet-fade-out' : 'animate-sheet-fade-in'}`}
      />
      <div
        className={`absolute inset-x-0 bottom-0 max-h-[85dvh] overflow-y-auto rounded-t-2xl bg-white shadow-xl ${closing ? 'animate-sheet-down' : 'animate-sheet-up'}`}
      >
        <div className="sticky top-0 z-10 flex justify-center bg-white pt-2 pb-1">
          <div className="h-1 w-10 rounded-full bg-gray-300" />
        </div>
        {children}
      </div>
    </div>
  )
}
