# Stats Analytics - Client Integration Guide

This guide explains how to integrate your website or application with the Stats analytics service to start collecting and analyzing user events.

## Overview

Stats is a self-hosted analytics service that collects events from your websites and applications. The system uses **collectors** to organize and track analytics data from different sources (domains, apps, etc.).

### Key Concepts

- **Collector**: A unique identifier that groups events from a specific source (website, app, etc.)
- **Event**: An action or pageview that gets tracked (e.g., page visit, button click, form submission)
- **Dashboard**: Real-time analytics interface to view your collected data

## Quick Start

### 1. Setup the Stats Service

First, ensure your Stats service is running with the proper configuration:

```bash
# Development mode
RUST_LOG=debug cargo run

# Production mode
cargo build --release
./target/release/stats
```

Make sure your `.env` file is configured:

```env
APP_URL=http://localhost:5775
SERVICE_PORT=5775
DATABASE_URL=data/stats.sqlite
CORS_DOMAINS=http://localhost:3000,https://yourdomain.com
PROCESSING_BATCH_SIZE=500
```

### 2. Automatic JavaScript Integration (Recommended)

The easiest way to start collecting analytics is to use the automatic JavaScript collector:

```html
<script>
  // Stats analytics
  var head = document.head || document.getElementsByTagName('head')[0];
  var script = document.createElement('script');
  script.setAttribute('src', 'http://your-stats-server.com/stats.js'); // Replace with your Stats server URL
  script.setAttribute('onload', () => window.collectStats());
  script.setAttribute('type', 'text/javascript');
  script.setAttribute('charset', 'utf8');
  script.setAttribute('async', '');
  head.appendChild(script);
</script>
```

**What this does:**

- Automatically creates a collector for your domain
- Tracks page views, navigation events, and user interactions
- Provides a `stats_collect()` function for custom events

### 3. Manual Collector Creation

If you need more control, you can create collectors manually:

```javascript
// Create a collector
const response = await fetch('http://your-stats-server.com/create-collector', {
  method: 'POST',
  headers: {
    Origin: 'https://yourdomain.com',
  },
});

const collector = await response.json();
console.log('Collector ID:', collector.id);
```

### 4. Logging Events

Once you have a collector, you can log events in several ways:

#### Using the Auto-Generated JavaScript Client

If you're using the automatic integration, you get access to the `stats_collect()` function:

```javascript
// Track a custom event
stats_collect('button_click');

// Track with custom URL
stats_collect('download', 'https://yourdomain.com/file.pdf');

// The following events are tracked automatically:
// - 'enter' - when user arrives on page
// - 'visit' - for navigation/page changes
// - 'leave' - when user clicks external links
// - 'exit' - when user leaves the site
```

#### Manual API Calls

You can also make direct API calls to log events:

```javascript
async function logEvent(collectorId, eventName, url, referrer = null) {
  const params = new URLSearchParams({
    collector_id: collectorId,
    name: eventName,
    url: url || window.location.href,
  });

  if (referrer) {
    params.set('referrer', referrer);
  }

  const response = await fetch(
    `http://your-stats-server.com/collect?${params}`,
  );
  return response.json();
}

// Usage
await logEvent('your-collector-id', 'signup', 'https://yourdomain.com/signup');
await logEvent(
  'your-collector-id',
  'purchase',
  'https://yourdomain.com/checkout',
);
```

#### Using HTTP Requests (Any Language)

```bash
# Log a page view
curl "http://your-stats-server.com/collect?collector_id=YOUR_COLLECTOR_ID&name=pageview&url=https://yourdomain.com/page"

# Log a custom event
curl "http://your-stats-server.com/collect?collector_id=YOUR_COLLECTOR_ID&name=button_click&url=https://yourdomain.com/landing&referrer=https://google.com"
```

## Event Types

### Standard Events (Auto-tracked)

When using the JavaScript integration, these events are automatically tracked:

- `enter` - User arrives on your site
- `visit` - Page navigation (including SPA route changes)
- `leave` - User clicks external links
- `exit` - User closes tab/navigates away

### Custom Events

You can track any custom events that are relevant to your application:

```javascript
// E-commerce events
stats_collect('product_view');
stats_collect('add_to_cart');
stats_collect('purchase');

// User engagement
stats_collect('video_play');
stats_collect('form_submit');
stats_collect('download');

// Custom business events
stats_collect('trial_started');
stats_collect('subscription_upgrade');
```

## Integration Examples

### React/Next.js Integration

```javascript
// utils/analytics.js
let statsCollect = null;

export const loadStats = () => {
  if (typeof window === 'undefined') return;

  const script = document.createElement('script');
  script.src = 'http://your-stats-server.com/stats.js';
  script.async = true;
  script.onload = () => {
    if (window.stats_collect) {
      statsCollect = window.stats_collect;
    }
  };
  document.head.appendChild(script);
};

export const trackEvent = (eventName, url = null) => {
  if (statsCollect) {
    statsCollect(eventName, url);
  }
};

// _app.js or layout.js
import { loadStats } from '../utils/analytics';

export default function App({ Component, pageProps }) {
  useEffect(() => {
    loadStats();
  }, []);

  return <Component {...pageProps} />;
}

// In your components
import { trackEvent } from '../utils/analytics';

const Button = () => {
  const handleClick = () => {
    trackEvent('cta_click');
    // Your button logic
  };

  return <button onClick={handleClick}>Sign Up</button>;
};
```

### Vue.js Integration

```javascript
// plugins/analytics.js
export default {
  install(app, options) {
    const script = document.createElement('script');
    script.src = options.statsUrl + '/stats.js';
    script.async = true;
    script.onload = () => {
      app.config.globalProperties.$stats = window.stats_collect;
    };
    document.head.appendChild(script);
  }
};

// main.js
import { createApp } from 'vue';
import Analytics from './plugins/analytics';

const app = createApp(App);
app.use(Analytics, { statsUrl: 'http://your-stats-server.com' });

// In components
export default {
  methods: {
    trackSignup() {
      this.$stats('user_signup');
    }
  }
};
```

### Mobile App Integration

For mobile apps, you'll need to make HTTP requests directly:

```javascript
// React Native example
const trackEvent = async (eventName, url, referrer = null) => {
  try {
    const params = new URLSearchParams({
      collector_id: 'your-collector-id',
      name: eventName,
      url: url,
    });

    if (referrer) params.set('referrer', referrer);

    await fetch(`http://your-stats-server.com/collect?${params}`);
  } catch (error) {
    console.error('Analytics error:', error);
  }
};

// Usage
await trackEvent('app_open', 'myapp://home');
await trackEvent('screen_view', 'myapp://profile');
```

## Best Practices

### 1. Event Naming Convention

Use consistent, descriptive event names:

```javascript
// Good
stats_collect('product_purchased');
stats_collect('newsletter_signup');
stats_collect('video_completed');

// Avoid
stats_collect('click');
stats_collect('event');
stats_collect('action');
```

### 2. Error Handling

Always handle analytics errors gracefully:

```javascript
const safeTrack = (eventName, url = null) => {
  try {
    if (window.stats_collect) {
      window.stats_collect(eventName, url);
    }
  } catch (error) {
    console.warn('Analytics tracking failed:', error);
    // Don't let analytics errors break your app
  }
};
```

### 3. Privacy Considerations

- The service automatically anonymizes IP addresses
- No cookies or persistent storage used
- Collectors are created per session/origin

### 4. Performance

- The JavaScript client is lightweight and loads asynchronously
- Events are queued and processed in batches
- Failed requests won't block your application

## Viewing Your Data

Once you're collecting events, you can view your analytics at:

```
http://your-stats-server.com/
```

The dashboard shows:

- Real-time visitor activity
- Top pages and referrers
- Browser and OS statistics
- Hourly activity patterns
- Geographic distribution (if GeoIP is configured)

## API Reference

### Collector Creation

```http
POST /create-collector
Headers: Origin: https://yourdomain.com
```

### Event Logging

```http
GET /collect?collector_id=ID&name=EVENT_NAME&url=URL&referrer=REFERRER
```

### JavaScript Client

```http
GET /stats.js
```

Returns a dynamically generated JavaScript client with an embedded collector ID.

## Troubleshooting

### Common Issues

1. **CORS Errors**: Make sure your domain is added to `CORS_DOMAINS` in the server configuration
2. **No Events Showing**: Check browser network tab for failed requests
3. **Collector Not Created**: Verify the `/stats.js` endpoint is accessible
4. **Events Not Appearing**: Check that the collector ID is being passed correctly

### Debug Mode

Enable debug logging in your browser console:

```javascript
// Add this to see detailed tracking information
window.addEventListener('load', () => {
  console.log('Stats collector loaded');
});
```

## Support

For issues and questions:

- Check the server logs for errors
- Verify your CORS configuration
- Test API endpoints directly using the examples in `src/api.http`

---

This guide should get you up and running with Stats analytics. The service is designed to be lightweight, privacy-focused, and easy to integrate with any web application or mobile app.
