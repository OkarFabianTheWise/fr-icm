# Shareable Pool Links

This feature allows pool creators to share their trading pools with others through social media and direct links.

## How it works

### 1. Creating a Pool

When a user successfully creates a trading pool, they will see a share dialog with:

- A copyable shareable link
- Social media sharing buttons (Twitter/X, Telegram, WhatsApp, LinkedIn)

### 2. Shareable Link Format

```
https://your-domain.com/pool/[poolName]/[creatorAddress]
```

### 3. Public Pool View

The shareable link opens a public page that shows:

- Pool details (name, strategy, assets, progress)
- Current fundraising status
- Contribution limits
- "Join This Pool" button (requires signup)

### 4. User Flow

1. **Visitors (not logged in)**: Can view all pool details but must sign up to contribute
2. **Clicking "Join This Pool"**: Redirects to main app with pool pre-selected
3. **Main app**: Handles authentication and contribution flow

## Features

### Share Dialog Components

- **Copy Link**: One-click copy to clipboard with visual feedback
- **Social Sharing**: Direct sharing to major platforms with pre-filled text
- **Pool Information**: Shows pool name and creation confirmation

### Public Pool Page

- **Responsive Design**: Works on mobile and desktop
- **Pool Analytics**: Shows progress, contributor count, time remaining
- **Asset Display**: Shows tradeable tokens with links to Solscan
- **Phase Indicators**: Visual status badges (Raising, Trading, Closed, Failed)

### Security & Privacy

- **View-only Access**: No authentication required for viewing
- **Contribution Gating**: Requires signup to participate
- **Creator Verification**: Pool ownership verified through blockchain data

## Technical Implementation

### Components

- `CreateView.tsx`: Handles share dialog after pool creation
- `SharedPoolPage.tsx`: Public pool view component
- `transactions.ts`: Pool data fetching and management

### URL Structure

```
/pool/[poolName]/[creator]
```

- `poolName`: URL-encoded pool name
- `creator`: URL-encoded creator wallet address

### API Integration

- Uses existing `getTradingPoolDetails` function
- Handles pool phase calculation
- Fetches real-time contribution data

## Usage Example

1. User creates pool "AI Arbitrage Strategy"
2. Success dialog shows shareable link: `/pool/AI%20Arbitrage%20Strategy/9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM`
3. Creator shares on Twitter with auto-generated text
4. Followers click link → see pool details → sign up to contribute

## Integration Points

### Main App

- Handle `?pool=...&creator=...` query parameters
- Pre-select shared pool when redirecting from public page
- Maintain existing authentication flow

### Social Platforms

- Twitter/X: Tweet with pool description and link
- Telegram: Share message with call-to-action
- WhatsApp: Personal message with pool details
- LinkedIn: Professional network sharing

This feature increases pool discoverability and enables viral growth through social sharing.
