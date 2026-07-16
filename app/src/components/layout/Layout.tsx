import { Outlet } from 'react-router-dom'
import NavBar from '../landing/NavBar'
import SiteFooter from '../landing/SiteFooter'

/**
 * Shared app shell — the Auros NavBar (with the real wallet connect + cluster
 * selector) above the routed page, with the SiteFooter beneath. Wraps every
 * route so the chrome is consistent across the landing, styleguide, and the
 * oracle browse views.
 */
export default function Layout() {
  return (
    <div className="flex min-h-screen flex-col bg-parchment">
      {/* Keyboard skip link — the first focusable element; hidden until focused,
          then jumps past the nav to the routed content. */}
      <a
        href="#main-content"
        className="sr-only rounded-button focus:not-sr-only focus:absolute focus:left-4 focus:top-4 focus:z-50 focus:bg-liquid-kelp focus:px-4 focus:py-2 focus:font-inter focus:text-[14px] focus:text-platinum focus:outline-none focus:ring-2 focus:ring-cyan-phosphor"
      >
        Skip to content
      </a>
      <NavBar />
      {/* Each routed page owns its own <main> landmark; this wrapper is the skip
          target and takes focus (tabIndex -1) when the skip link is used. */}
      <div id="main-content" tabIndex={-1} className="flex-1 outline-none">
        <Outlet />
      </div>
      <SiteFooter />
    </div>
  )
}
