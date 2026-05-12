import type { Metadata } from 'next';
import { GoogleAnalytics } from '@/components/GoogleAnalytics';
import { SiteFooter } from '@/components/SiteFooter';
import 'bootstrap/dist/css/bootstrap-grid.min.css';
import 'bootstrap/dist/css/bootstrap-utilities.min.css';
import 'bootstrap-icons/font/bootstrap-icons.css';
import './globals.css';

export const metadata: Metadata = {
  title: 'CHIPTUNOMATIC',
  description: 'Convert any file into chiptune music',
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body>
        <GoogleAnalytics />
        <div className="site-shell d-flex flex-column min-vh-100">
          <div className="site-content flex-fill d-flex flex-column">{children}</div>
          <SiteFooter />
        </div>
      </body>
    </html>
  );
}
