"""Earnings Call Transcript NLP Service.

Fetches SEC EDGAR 8-K filings and analyzes management commentary
for tone, guidance sentiment, and key topic extraction.
Runs on port 8005.
"""
import logging
import os
import re
import sys
import time
from pathlib import Path
from typing import Dict, List, Optional

from fastapi import FastAPI, HTTPException
from pydantic import BaseModel

# Add parent directory to path for shared imports
sys.path.append(str(Path(__file__).parent.parent))

logger = logging.getLogger(__name__)

app = FastAPI(title="Earnings Call Transcript NLP Service", version="1.0.0")

# Production hardening
try:
    from shared.middleware import setup_hardening
    setup_hardening(app, "earnings-nlp")
except ImportError:
    logger.warning("shared.middleware not available, skipping hardening")


# ---------------------------------------------------------------------------
# Pydantic Models
# ---------------------------------------------------------------------------
class TranscriptAnalysisRequest(BaseModel):
    symbol: str
    text: Optional[str] = None  # If provided, analyze this text directly


class TranscriptAnalysisResponse(BaseModel):
    symbol: str
    overall_tone: str  # positive, negative, neutral, mixed
    tone_score: float  # -1.0 to 1.0
    confidence: float  # 0.0 to 1.0
    key_topics: List[str]
    guidance_sentiment: str  # raised, maintained, lowered, not_mentioned
    guidance_keywords: List[str]
    tone_shift: Optional[str]  # more_positive, more_negative, stable, unknown
    forward_looking_count: int
    risk_mentions: int
    data_source: str
    processing_time_ms: float


class HealthResponse(BaseModel):
    status: str
    version: str


# ---------------------------------------------------------------------------
# NLP Analysis Engine (word-list based, no heavy ML dependency)
# ---------------------------------------------------------------------------
POSITIVE_WORDS = {
    "strong", "growth", "exceed", "exceeded", "beat", "record", "momentum",
    "robust", "outperform", "outperformed", "improved", "improving", "optimistic",
    "confident", "accelerat", "expand", "expansion", "upside", "favorable",
    "progress", "achieved", "strength", "profitable", "profitability",
    "opportunity", "opportunities", "innovative", "innovation", "efficiency",
    "margin expansion", "raised", "raising", "upgrade", "upgraded", "tailwind",
    "resilient", "resilience", "surpass", "surpassed", "positive", "above",
    "higher", "increase", "increased", "increasing", "gain", "gains",
    "dividend", "buyback", "repurchase",
}

NEGATIVE_WORDS = {
    "weak", "decline", "declined", "declining", "miss", "missed", "shortfall",
    "headwind", "headwinds", "challenge", "challenges", "challenging",
    "uncertainty", "uncertain", "pressure", "pressured", "difficult",
    "disappointing", "disappointed", "deteriorat", "slower", "slowdown",
    "downturn", "risk", "risks", "cautious", "conservative", "impairment",
    "restructuring", "layoff", "layoffs", "litigation", "loss", "losses",
    "below", "lower", "decrease", "decreased", "decreasing", "concern",
    "concerned", "warning", "warned", "downside", "adverse", "negative",
    "contraction", "margin compression", "lowered", "lowering", "downgrade",
    "downgraded", "recession", "inflationary", "inflation",
}

GUIDANCE_KEYWORDS = {
    "guidance", "outlook", "forecast", "expect", "expects", "expecting",
    "anticipated", "anticipate", "anticipates", "project", "projected",
    "projection", "target", "targets", "range", "full-year", "full year",
    "quarter", "fiscal year", "fy", "q1", "q2", "q3", "q4",
    "revenue guidance", "earnings guidance", "eps guidance",
}

GUIDANCE_RAISED_WORDS = {"raised", "raising", "increase", "above", "higher", "upward", "upgraded"}
GUIDANCE_LOWERED_WORDS = {"lowered", "lowering", "decrease", "below", "lower", "downward", "cut", "reduced"}

FORWARD_LOOKING_PHRASES = {
    "going forward", "looking ahead", "next quarter", "next year",
    "we expect", "we anticipate", "we believe", "we plan", "we intend",
    "our outlook", "our guidance", "our forecast", "our target",
    "in the coming", "for the remainder", "second half",
    "pipeline", "backlog", "order book", "visibility",
}

RISK_PHRASES = {
    "risk factor", "material adverse", "litigation", "regulatory",
    "compliance", "investigation", "impairment", "write-down",
    "write-off", "going concern", "default", "covenant",
    "supply chain", "cybersecurity", "data breach",
}

# Simple SEC EDGAR fetcher
EDGAR_BASE = "https://efts.sec.gov/LATEST/search-index?q=%22earnings%22&dateRange=custom&startdt={start}&enddt={end}&forms=8-K&tickers={symbol}"
EDGAR_FILING_URL = "https://www.sec.gov/cgi-bin/browse-edgar?action=getcompany&CIK={symbol}&type=8-K&dateb=&owner=include&count=5&search_text=&action=getcompany"


def _fetch_edgar_filings(symbol: str) -> Optional[str]:
    """Attempt to fetch recent 8-K filing text from SEC EDGAR full-text search."""
    try:
        import requests
        headers = {"User-Agent": "InvestIQ/1.0 research@investiq.dev"}

        # Use EDGAR full-text search for recent earnings filings
        url = f"https://efts.sec.gov/LATEST/search-index?q=%22earnings%22+%22{symbol}%22&forms=8-K&dateRange=custom"
        resp = requests.get(url, headers=headers, timeout=10)
        if resp.status_code == 200:
            data = resp.json()
            hits = data.get("hits", {}).get("hits", [])
            if hits:
                # Get the first filing document URL
                filing_url = hits[0].get("_source", {}).get("file_url")
                if filing_url:
                    doc_resp = requests.get(
                        f"https://www.sec.gov{filing_url}",
                        headers=headers,
                        timeout=15,
                    )
                    if doc_resp.status_code == 200:
                        return doc_resp.text[:50000]  # Cap at 50k chars
    except Exception as e:
        logger.debug("EDGAR fetch failed for %s: %s", symbol, e)
    return None


def analyze_transcript_text(text: str) -> Dict:
    """Analyze a transcript/filing text using word-list NLP."""
    text_lower = text.lower()
    words = set(re.findall(r'\b\w+\b', text_lower))
    sentences = re.split(r'[.!?]+', text_lower)

    # Sentiment scoring
    pos_count = sum(1 for w in POSITIVE_WORDS if w in text_lower)
    neg_count = sum(1 for w in NEGATIVE_WORDS if w in text_lower)
    total = pos_count + neg_count
    if total > 0:
        tone_score = (pos_count - neg_count) / total
    else:
        tone_score = 0.0

    confidence = min(total / 30.0, 1.0)  # More words matched = higher confidence

    if tone_score > 0.2:
        overall_tone = "positive"
    elif tone_score < -0.2:
        overall_tone = "negative"
    elif abs(tone_score) <= 0.2 and total > 5:
        overall_tone = "mixed"
    else:
        overall_tone = "neutral"

    # Key topics extraction (most frequent meaningful terms)
    topic_words = {}
    for word in re.findall(r'\b[a-z]{4,}\b', text_lower):
        if word not in {"that", "this", "with", "have", "from", "been", "were", "will",
                        "they", "their", "which", "about", "would", "could", "should",
                        "what", "when", "there", "other", "more", "than", "also", "into",
                        "some", "only", "over", "such", "after", "before", "each", "between"}:
            topic_words[word] = topic_words.get(word, 0) + 1
    top_topics = sorted(topic_words.items(), key=lambda x: -x[1])[:10]
    key_topics = [w for w, _ in top_topics]

    # Guidance sentiment
    guidance_found = [kw for kw in GUIDANCE_KEYWORDS if kw in text_lower]
    if guidance_found:
        # Check guidance context
        raised_count = sum(1 for w in GUIDANCE_RAISED_WORDS if w in text_lower)
        lowered_count = sum(1 for w in GUIDANCE_LOWERED_WORDS if w in text_lower)
        if raised_count > lowered_count:
            guidance_sentiment = "raised"
        elif lowered_count > raised_count:
            guidance_sentiment = "lowered"
        else:
            guidance_sentiment = "maintained"
    else:
        guidance_sentiment = "not_mentioned"

    # Forward-looking statements
    forward_count = sum(1 for phrase in FORWARD_LOOKING_PHRASES if phrase in text_lower)

    # Risk mentions
    risk_count = sum(1 for phrase in RISK_PHRASES if phrase in text_lower)

    return {
        "overall_tone": overall_tone,
        "tone_score": round(tone_score, 4),
        "confidence": round(confidence, 4),
        "key_topics": key_topics,
        "guidance_sentiment": guidance_sentiment,
        "guidance_keywords": guidance_found[:5],
        "forward_looking_count": forward_count,
        "risk_mentions": risk_count,
    }


# ---------------------------------------------------------------------------
# Transcript cache (symbol -> result, 60min TTL)
# ---------------------------------------------------------------------------
_cache: Dict[str, tuple] = {}  # symbol -> (timestamp, result)
CACHE_TTL = 3600  # 60 minutes


def _get_cached(symbol: str) -> Optional[Dict]:
    if symbol in _cache:
        ts, result = _cache[symbol]
        if time.time() - ts < CACHE_TTL:
            return result
        del _cache[symbol]
    return None


def _set_cache(symbol: str, result: Dict):
    _cache[symbol] = (time.time(), result)


# ---------------------------------------------------------------------------
# Endpoints
# ---------------------------------------------------------------------------
@app.get("/health", response_model=HealthResponse)
async def health():
    return HealthResponse(status="healthy", version="1.0.0")


@app.get("/earnings-nlp/{symbol}", response_model=TranscriptAnalysisResponse)
async def analyze_earnings(symbol: str):
    """Analyze the most recent earnings filing for a symbol."""
    start_time = time.time()
    symbol = symbol.upper().strip()

    # Check cache
    cached = _get_cached(symbol)
    if cached:
        cached["processing_time_ms"] = (time.time() - start_time) * 1000
        return TranscriptAnalysisResponse(**cached)

    # Try fetching from EDGAR
    text = _fetch_edgar_filings(symbol)
    data_source = "sec_edgar"

    if not text:
        # Return empty analysis when no transcript available
        return TranscriptAnalysisResponse(
            symbol=symbol,
            overall_tone="unknown",
            tone_score=0.0,
            confidence=0.0,
            key_topics=[],
            guidance_sentiment="not_mentioned",
            guidance_keywords=[],
            tone_shift=None,
            forward_looking_count=0,
            risk_mentions=0,
            data_source="none",
            processing_time_ms=(time.time() - start_time) * 1000,
        )

    analysis = analyze_transcript_text(text)
    result = {
        "symbol": symbol,
        "data_source": data_source,
        "tone_shift": None,  # Would need previous quarter's data
        **analysis,
    }

    _set_cache(symbol, result)

    result["processing_time_ms"] = (time.time() - start_time) * 1000
    return TranscriptAnalysisResponse(**result)


@app.post("/analyze-transcript", response_model=TranscriptAnalysisResponse)
async def analyze_transcript(request: TranscriptAnalysisRequest):
    """Analyze provided transcript text directly."""
    start_time = time.time()
    symbol = request.symbol.upper().strip()

    if not request.text or len(request.text.strip()) < 50:
        raise HTTPException(status_code=400, detail="Transcript text must be at least 50 characters")

    analysis = analyze_transcript_text(request.text)
    result = TranscriptAnalysisResponse(
        symbol=symbol,
        data_source="user_provided",
        tone_shift=None,
        processing_time_ms=(time.time() - start_time) * 1000,
        **analysis,
    )
    return result


if __name__ == "__main__":
    import uvicorn
    port = int(os.getenv("EARNINGS_NLP_PORT", "8005"))
    uvicorn.run(app, host="0.0.0.0", port=port, log_level="info")
