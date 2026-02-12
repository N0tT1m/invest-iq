# Mobile-Responsive Frontend TODO

Checklist for making the Dash frontend responsive across mobile, tablet, and desktop viewports.

## Layout & Breakpoints
- [ ] Add Bootstrap responsive breakpoints to all component layouts (`md=12` on mobile, `md=6` on tablet)
- [ ] Make chart heights responsive (`style={"height": "300px"}` to percentage or vh-based)
- [ ] Collapse sidebar/navigation into hamburger menu on mobile
- [ ] Make trading tabs horizontally scrollable on small screens
- [ ] Add `<meta name="viewport" content="width=device-width, initial-scale=1">` to `index_string`

## Data Tables
- [ ] Reduce data table columns on mobile (show symbol + signal + confidence only)
- [ ] Add horizontal scroll for wide tables on small screens

## Touch & Interaction
- [ ] Add touch-friendly tap targets (min 44px) for buttons and interactive elements
- [ ] Test Plotly chart touch interactions (pinch-zoom, pan)

## Testing
- [ ] Test all 21 components at 375px, 768px, 1024px, 1440px viewport widths
- [ ] Test on iOS Safari and Android Chrome
- [ ] Verify no horizontal overflow at any breakpoint

## Performance
- [ ] Add loading skeletons for slow-loading components
- [ ] Optimize chart rendering for mobile GPU (reduce data points on small screens)

## PWA (Optional)
- [ ] Consider PWA manifest + service worker for add-to-home-screen
- [ ] Add offline fallback page
