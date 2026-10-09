import { Component, type ErrorInfo, type ReactNode } from 'react'
import { AlertCircle, RotateCcw } from 'lucide-react'

interface ErrorBoundaryProps {
  children: ReactNode
}

interface ErrorBoundaryState {
  hasError: boolean
  error: Error | null
}

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  constructor(props: ErrorBoundaryProps) {
    super(props)
    this.state = { hasError: false, error: null }
  }

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { hasError: true, error }
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error('ErrorBoundary caught an error:', error, errorInfo)
  }

  handleReset = () => {
    this.setState({ hasError: false, error: null })
    window.location.reload()
  }

  render() {
    if (this.state.hasError) {
      return (
        <div className="w-full h-full min-h-35 flex flex-col items-center justify-center p-4 bg-[#161618] border border-red-500/30 rounded-2xl text-zinc-200 shadow-2xl">
          <div className="flex items-center gap-2 text-red-400 mb-2">
            <AlertCircle className="size-5" />
            <span className="text-sm font-semibold">Render Error Encountered</span>
          </div>
          <p className="text-xs text-zinc-400 mb-4 text-center max-w-sm">
            An unexpected error occurred while rendering the view. Click below to recover.
          </p>
          <button
            onClick={this.handleReset}
            type="button"
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-white/10 hover:bg-white/20 text-xs font-medium text-white transition-colors"
          >
            <RotateCcw className="size-3.5" />
            <span>Recover</span>
          </button>
        </div>
      )
    }

    return this.props.children
  }
}
