'use client';

import { useEffect } from 'react';
import { usePathname } from 'next/navigation';
import ReactGA from 'react-ga';

const trackingId = process.env.NEXT_PUBLIC_GA_MEASUREMENT_ID;

let gaInitialized = false;

export function GoogleAnalytics() {
  const pathname = usePathname();

  useEffect(() => {
    if (!trackingId) return;
    if (!gaInitialized) {
      console.log('Initialize GA');
      ReactGA.initialize(trackingId, { titleCase: false });
      gaInitialized = true;
    }
    const search = typeof window !== 'undefined' ? window.location.search : '';
    ReactGA.pageview(`${pathname}${search}`);
  }, [pathname, trackingId]);

  return null;
}
