import { error } from '@sveltejs/kit';
import { clickmanSections, type ClickmanRoute, type ClickmanSection } from '@clickman/dashboard';
import type { PageLoad } from './$types';

export const load: PageLoad = ({ params }) => {
  const parts = params.path ? params.path.split('/') : [];
  if (!parts.length)
    return { route: { projectId: null, section: 'overview' } satisfies ClickmanRoute };
  if (
    parts.length !== 2 ||
    !/^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i.test(parts[0]) ||
    !clickmanSections.includes(parts[1] as ClickmanSection)
  )
    error(404, 'Unknown analytics page.');
  return {
    route: {
      projectId: parts[0].toLowerCase(),
      section: parts[1] as ClickmanSection,
    } satisfies ClickmanRoute,
  };
};
