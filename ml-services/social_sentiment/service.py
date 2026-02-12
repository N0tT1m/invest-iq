"""FastAPI service for Social Media Sentiment analysis (Reddit).

Port 8006. Provides /social-sentiment/{symbol}, /health.
"""
import logging
import time
from typing import Optional

from fastapi import FastAPI, HTTPException
from pydantic import BaseModel, Field
import uvicorn

logger = logging.getLogger(__name__)

app = FastAPI(title="Social Sentiment Service", version="1.0.0")

try:
    from shared.middleware import setup_hardening
    setup_hardening(app, "social-sentiment")
except ImportError:
    pass

# Lazy-loaded Reddit client
_reddit_client = None


def get_reddit():
    global _reddit_client
    if _reddit_client is not None:
        return _reddit_client
    try:
        import praw
        import os
        client_id = os.environ.get("REDDIT_CLIENT_ID", "")
        client_secret = os.environ.get("REDDIT_CLIENT_SECRET", "")
        user_agent = os.environ.get("REDDIT_USER_AGENT", "InvestIQ/1.0")
        if not client_id or not client_secret:
            logger.warning("Reddit API credentials not set (REDDIT_CLIENT_ID, REDDIT_CLIENT_SECRET)")
            return None
        _reddit_client = praw.Reddit(
            client_id=client_id,
            client_secret=client_secret,
            user_agent=user_agent,
        )
        logger.info("Reddit API client initialized")
        return _reddit_client
    except ImportError:
        logger.warning("praw not installed - Reddit sentiment unavailable")
        return None
    except Exception as e:
        logger.error(f"Failed to init Reddit client: {e}")
        return None


# Simple word-list fallback scorer
POSITIVE_WORDS = {"bull", "buy", "moon", "rocket", "calls", "up", "gain", "profit",
                  "green", "breakout", "squeeze", "long", "diamond", "hodl", "yolo"}
NEGATIVE_WORDS = {"bear", "sell", "put", "down", "loss", "crash", "dump", "short",
                  "red", "drop", "tank", "baghold", "margin", "rekt", "overvalued"}


def score_text(text: str) -> float:
    words = set(text.lower().split())
    pos = len(words & POSITIVE_WORDS)
    neg = len(words & NEGATIVE_WORDS)
    total = pos + neg
    if total == 0:
        return 0.5
    return pos / total


class SentimentResponse(BaseModel):
    symbol: str
    mentions: int = 0
    avg_sentiment: float = 0.5
    sentiment_label: str = "neutral"
    buzz_level: str = "low"
    trending: bool = False
    top_posts: list = Field(default_factory=list)
    subreddits_searched: list = Field(default_factory=list)
    data_source: str = "reddit"


SUBREDDITS = ["wallstreetbets", "stocks", "investing", "options"]


@app.get("/social-sentiment/{symbol}", response_model=SentimentResponse)
async def get_social_sentiment(symbol: str, limit: int = 50):
    symbol = symbol.upper()
    reddit = get_reddit()

    if reddit is None:
        return SentimentResponse(
            symbol=symbol,
            data_source="unavailable",
            subreddits_searched=[],
        )

    all_posts = []
    sentiments = []
    subreddits_searched = []

    for sub_name in SUBREDDITS:
        try:
            subreddit = reddit.subreddit(sub_name)
            for post in subreddit.search(symbol, sort="new", time_filter="week", limit=limit):
                text = f"{post.title} {post.selftext[:500]}"
                sentiment = score_text(text)
                sentiments.append(sentiment)
                all_posts.append({
                    "title": post.title[:120],
                    "subreddit": sub_name,
                    "score": post.score,
                    "num_comments": post.num_comments,
                    "sentiment": round(sentiment, 2),
                    "url": f"https://reddit.com{post.permalink}",
                })
            subreddits_searched.append(sub_name)
        except Exception as e:
            logger.warning(f"Error searching r/{sub_name} for {symbol}: {e}")

    mentions = len(all_posts)
    avg_sentiment = sum(sentiments) / len(sentiments) if sentiments else 0.5

    # Buzz level based on mention count
    if mentions >= 50:
        buzz_level = "very_high"
    elif mentions >= 20:
        buzz_level = "high"
    elif mentions >= 5:
        buzz_level = "moderate"
    else:
        buzz_level = "low"

    trending = mentions >= 20

    # Label
    if avg_sentiment >= 0.65:
        label = "bullish"
    elif avg_sentiment >= 0.55:
        label = "slightly_bullish"
    elif avg_sentiment <= 0.35:
        label = "bearish"
    elif avg_sentiment <= 0.45:
        label = "slightly_bearish"
    else:
        label = "neutral"

    # Sort by Reddit score descending, take top 10
    top_posts = sorted(all_posts, key=lambda p: p.get("score", 0), reverse=True)[:10]

    return SentimentResponse(
        symbol=symbol,
        mentions=mentions,
        avg_sentiment=round(avg_sentiment, 3),
        sentiment_label=label,
        buzz_level=buzz_level,
        trending=trending,
        top_posts=top_posts,
        subreddits_searched=subreddits_searched,
    )


@app.get("/health")
async def health():
    reddit = get_reddit()
    return {
        "status": "healthy",
        "service": "social-sentiment",
        "reddit_available": reddit is not None,
    }


if __name__ == "__main__":
    logging.basicConfig(level=logging.INFO)
    uvicorn.run(app, host="0.0.0.0", port=8006)
