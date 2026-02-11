"""FinBERT Fine-tuning Script.

Data sources (in priority order):
  1. --use-polygon: Fetch news from Polygon API, label with actual price returns
  2. --dataset: Custom CSV with columns (text, label)
  3. --use-public: HuggingFace Financial PhraseBank (fallback)
"""
import os
from dotenv import load_dotenv
load_dotenv()                       # ml-services/.env
load_dotenv(dotenv_path="../.env")  # project root .env
import time
import torch
import requests
import numpy as np
import pandas as pd
from transformers import (
    AutoTokenizer,
    AutoModelForSequenceClassification,
    Trainer,
    TrainingArguments,
    DataCollatorWithPadding,
)
from datasets import Dataset, load_dataset
from pathlib import Path
from datetime import datetime, timedelta
import argparse
import sys
from loguru import logger
from sklearn.metrics import accuracy_score, precision_recall_fscore_support

sys.path.append(str(Path(__file__).parent.parent))
from shared.config import config
from shared.database import MLDatabase


# ---- Polygon News + Price Data ----

POLYGON_BASE = "https://api.polygon.io"

DEFAULT_SYMBOLS = [
    "AAPL", "MSFT", "GOOGL", "NVDA", "META", "AMZN", "TSLA",
    "JPM", "BAC", "GS", "JNJ", "UNH", "PFE", "XOM", "CVX",
    "HD", "NKE", "KO", "PG", "DIS", "NFLX", "V", "MA",
    "CAT", "BA", "NEE", "SPG", "LIN",
]


def _polygon_get(path: str, params: dict, api_key: str) -> dict:
    """Make a Polygon API request."""
    params["apiKey"] = api_key
    resp = requests.get(f"{POLYGON_BASE}{path}", params=params, timeout=30)
    resp.raise_for_status()
    return resp.json()


def fetch_polygon_news(
    symbols: list[str],
    days_back: int = 90,
    api_key: str = "",
    delay: float = 0.25,
) -> list[dict]:
    """Fetch news articles from Polygon for multiple symbols (with pagination)."""
    end = datetime.now()
    start = end - timedelta(days=days_back)

    all_articles = []
    seen_ids = set()

    for symbol in symbols:
        symbol_count = 0
        try:
            # First page
            params = {
                "ticker": symbol,
                "published_utc.gte": start.strftime("%Y-%m-%d"),
                "published_utc.lte": end.strftime("%Y-%m-%d"),
                "limit": 1000,
                "sort": "published_utc",
            }
            data = _polygon_get("/v2/reference/news", params, api_key)

            while True:
                for article in data.get("results", []):
                    aid = article.get("id", article.get("title", ""))
                    if aid in seen_ids:
                        continue
                    seen_ids.add(aid)
                    all_articles.append({
                        "id": aid,
                        "title": article.get("title", ""),
                        "description": article.get("description", ""),
                        "published_utc": article.get("published_utc", ""),
                        "tickers": article.get("tickers", []),
                        "primary_symbol": symbol,
                    })
                    symbol_count += 1

                # Follow pagination cursor
                next_url = data.get("next_url")
                if not next_url:
                    break
                sep = "&" if "?" in next_url else "?"
                resp = requests.get(f"{next_url}{sep}apiKey={api_key}", timeout=30)
                resp.raise_for_status()
                data = resp.json()
                time.sleep(delay)

            logger.info(f"  {symbol}: {symbol_count} articles")
        except Exception as e:
            logger.warning(f"  {symbol}: news fetch failed - {e}")

        time.sleep(delay)

    logger.info(f"Total unique articles: {len(all_articles)}")
    return all_articles


def fetch_price_change(symbol: str, date_str: str, days_forward: int, api_key: str) -> float | None:
    """Get price change N days after a date using Polygon bars."""
    try:
        pub_date = datetime.fromisoformat(date_str.replace("Z", "+00:00"))
        start = pub_date.strftime("%Y-%m-%d")
        end = (pub_date + timedelta(days=days_forward + 5)).strftime("%Y-%m-%d")

        data = _polygon_get(
            f"/v2/aggs/ticker/{symbol}/range/1/day/{start}/{end}",
            {"adjusted": "true", "sort": "asc", "limit": days_forward + 5},
            api_key,
        )

        results = data.get("results", [])
        if len(results) < 2:
            return None

        open_price = results[0]["c"]  # close on publish day
        # Use the bar closest to days_forward trading days later
        idx = min(days_forward, len(results) - 1)
        close_price = results[idx]["c"]

        return (close_price - open_price) / open_price * 100.0
    except Exception:
        return None


def build_polygon_dataset(
    symbols: list[str],
    days_back: int = 90,
    return_horizon: int = 5,
    api_key: str = "",
    delay: float = 0.25,
) -> pd.DataFrame:
    """Build labeled sentiment dataset from Polygon news + actual price returns.

    Label mapping (FinBERT format):
      0 = positive (price went up > 0.5%)
      1 = negative (price went down < -0.5%)
      2 = neutral  (price stayed within +/-0.5%)
    """
    articles = fetch_polygon_news(symbols, days_back, api_key, delay)

    rows = []
    for i, article in enumerate(articles):
        text = article["title"]
        desc = article.get("description", "")
        if desc:
            text = f"{text}. {desc[:200]}"

        symbol = article["primary_symbol"]
        pub = article["published_utc"]

        pct = fetch_price_change(symbol, pub, return_horizon, api_key)
        if pct is None:
            continue

        # Label based on actual price movement
        if pct > 0.5:
            label = 0  # positive
        elif pct < -0.5:
            label = 1  # negative
        else:
            label = 2  # neutral

        rows.append({"text": text, "label": label, "return_pct": pct, "symbol": symbol})

        if (i + 1) % 50 == 0:
            logger.info(f"  Labeled {i + 1}/{len(articles)} articles ({len(rows)} valid)")

        time.sleep(delay)

    logger.info(f"Built dataset: {len(rows)} labeled samples from {len(articles)} articles")

    if not rows:
        raise ValueError("No labeled samples generated. Check Polygon API key and data availability.")

    df = pd.DataFrame(rows)

    # Log label distribution
    for label, name in [(0, "positive"), (1, "negative"), (2, "neutral")]:
        count = len(df[df["label"] == label])
        logger.info(f"  {name}: {count} ({count/len(df)*100:.1f}%)")

    return df


# ---- Trainer ----

class FinBERTTrainer:
    """Fine-tune FinBERT on financial sentiment data."""

    def __init__(
        self,
        model_name: str = "ProsusAI/finbert",
        output_dir: str = "./models/sentiment/fine-tuned",
        learning_rate: float = 2e-5,
        epochs: int = 3,
        batch_size: int = 16,
        warmup_steps: int = 500,
        weight_decay: float = 0.01,
    ):
        self.model_name = model_name
        self.output_dir = Path(output_dir)
        self.output_dir.mkdir(parents=True, exist_ok=True)
        self.learning_rate = learning_rate
        self.epochs = epochs
        self.batch_size = batch_size
        self.warmup_steps = warmup_steps
        self.weight_decay = weight_decay

        logger.info(f"Initializing FinBERT trainer with model: {model_name}")

    def prepare_dataset(
        self,
        dataset_path: str = None,
        use_public_data: bool = False,
        polygon_df: pd.DataFrame = None,
    ):
        """Prepare training dataset from Polygon data, CSV, or public datasets."""
        if polygon_df is not None:
            dataset = Dataset.from_pandas(polygon_df[["text", "label"]])
            logger.info(f"Using Polygon-labeled dataset: {len(dataset)} samples")

        elif dataset_path:
            df = pd.read_csv(dataset_path)
            dataset = Dataset.from_pandas(df)
            logger.info(f"Loaded custom dataset: {len(dataset)} samples")

        elif use_public_data:
            logger.info("Loading Financial PhraseBank dataset...")
            dataset = load_dataset("financial_phrasebank", "sentences_allagree", split="train")

            # Remap: PhraseBank (0=neg,1=neut,2=pos) -> FinBERT (0=pos,1=neg,2=neut)
            def remap_labels(example):
                label_map = {0: 1, 1: 2, 2: 0}
                example["label"] = label_map[example["label"]]
                return example

            dataset = dataset.map(remap_labels)
            logger.info(f"Loaded public dataset: {len(dataset)} samples")

        else:
            raise ValueError("Must provide polygon_df, dataset_path, or set use_public_data=True")

        split = dataset.train_test_split(test_size=0.2, seed=42)
        logger.info(f"Train: {len(split['train'])}, Eval: {len(split['test'])}")
        return split["train"], split["test"]

    def tokenize_dataset(self, dataset, tokenizer):
        """Tokenize dataset."""
        def tokenize_fn(examples):
            text_col = "text" if "text" in examples else "sentence"
            return tokenizer(examples[text_col], padding=False, truncation=True, max_length=512)

        return dataset.map(tokenize_fn, batched=True, remove_columns=dataset.column_names)

    def compute_metrics(self, eval_pred):
        """Compute evaluation metrics."""
        predictions, labels = eval_pred
        predictions = np.argmax(predictions, axis=1)
        accuracy = accuracy_score(labels, predictions)
        precision, recall, f1, _ = precision_recall_fscore_support(labels, predictions, average="weighted")
        return {"accuracy": accuracy, "f1": f1, "precision": precision, "recall": recall}

    def train(self, train_dataset, eval_dataset):
        """Fine-tune the model."""
        logger.info("Starting training...")

        tokenizer = AutoTokenizer.from_pretrained(self.model_name)
        model = AutoModelForSequenceClassification.from_pretrained(self.model_name, num_labels=3)

        train_tokenized = self.tokenize_dataset(train_dataset, tokenizer)
        eval_tokenized = self.tokenize_dataset(eval_dataset, tokenizer)

        data_collator = DataCollatorWithPadding(tokenizer=tokenizer)

        training_args = TrainingArguments(
            output_dir=str(self.output_dir),
            learning_rate=self.learning_rate,
            per_device_train_batch_size=self.batch_size,
            per_device_eval_batch_size=self.batch_size,
            num_train_epochs=self.epochs,
            weight_decay=self.weight_decay,
            warmup_steps=self.warmup_steps,
            evaluation_strategy="epoch",
            save_strategy="epoch",
            load_best_model_at_end=True,
            metric_for_best_model="f1",
            push_to_hub=False,
            logging_dir=str(self.output_dir / "logs"),
            logging_steps=100,
            fp16=torch.cuda.is_available(),
            dataloader_num_workers=4,
            remove_unused_columns=True,
        )

        trainer = Trainer(
            model=model,
            args=training_args,
            train_dataset=train_tokenized,
            eval_dataset=eval_tokenized,
            tokenizer=tokenizer,
            data_collator=data_collator,
            compute_metrics=self.compute_metrics,
        )

        logger.info("Training model...")
        train_result = trainer.train()

        logger.info("Evaluating model...")
        eval_result = trainer.evaluate()

        logger.info(f"Saving model to {self.output_dir}")
        trainer.save_model(str(self.output_dir))
        tokenizer.save_pretrained(str(self.output_dir))

        logger.info(f"Training metrics: {train_result.metrics}")
        logger.info(f"Evaluation metrics: {eval_result}")

        db = MLDatabase(config.database_path)
        db.save_model_metadata(
            model_name="finbert_sentiment",
            model_type="sentiment",
            version="v1",
            path=str(self.output_dir),
            metrics=eval_result,
            config={
                "base_model": self.model_name,
                "learning_rate": self.learning_rate,
                "epochs": self.epochs,
                "batch_size": self.batch_size,
            },
        )

        return eval_result


def main():
    parser = argparse.ArgumentParser(description="Fine-tune FinBERT on financial sentiment data")
    parser.add_argument("--dataset", type=str, help="Path to custom dataset CSV (text, label columns)")
    parser.add_argument("--use-public", action="store_true", help="Use HuggingFace Financial PhraseBank (fallback)")
    parser.add_argument("--use-polygon", action="store_true",
                       help="Fetch news from Polygon, label with actual price returns (recommended)")
    parser.add_argument("--symbols", nargs="+", default=None,
                       help="Symbols to fetch news for (default: 28 diversified stocks)")
    parser.add_argument("--days-back", type=int, default=90, help="Days of news history to fetch")
    parser.add_argument("--return-horizon", type=int, default=5, help="Days forward to measure price return")
    parser.add_argument("--output-dir", type=str, default="./models/sentiment/fine-tuned")
    parser.add_argument("--epochs", type=int, default=3)
    parser.add_argument("--batch-size", type=int, default=16)
    parser.add_argument("--learning-rate", type=float, default=2e-5)
    parser.add_argument("--db-path", type=str, default="../portfolio.db", help="Path to portfolio.db")

    args = parser.parse_args()

    trainer = FinBERTTrainer(
        output_dir=args.output_dir,
        learning_rate=args.learning_rate,
        epochs=args.epochs,
        batch_size=args.batch_size,
    )

    polygon_df = None

    if args.use_polygon:
        api_key = os.environ.get("POLYGON_API_KEY", "")
        if not api_key:
            logger.error("POLYGON_API_KEY env var not set. Use --use-public as fallback.")
            sys.exit(1)

        symbols = args.symbols or DEFAULT_SYMBOLS
        logger.info(f"Building dataset from Polygon news ({len(symbols)} symbols, {args.days_back}d lookback)")
        polygon_df = build_polygon_dataset(
            symbols=symbols,
            days_back=args.days_back,
            return_horizon=args.return_horizon,
            api_key=api_key,
        )
        # Save labeled data for reuse
        cache_path = Path(args.output_dir) / "polygon_training_data.csv"
        cache_path.parent.mkdir(parents=True, exist_ok=True)
        polygon_df.to_csv(cache_path, index=False)
        logger.info(f"Cached labeled data to {cache_path}")

    # If no explicit source, default to polygon if key is available, else public
    if not args.use_polygon and not args.dataset and not args.use_public:
        api_key = os.environ.get("POLYGON_API_KEY", "")
        if api_key:
            logger.info("POLYGON_API_KEY found, defaulting to --use-polygon")
            symbols = args.symbols or DEFAULT_SYMBOLS
            polygon_df = build_polygon_dataset(
                symbols=symbols,
                days_back=args.days_back,
                return_horizon=args.return_horizon,
                api_key=api_key,
            )
        else:
            logger.info("No data source specified and no POLYGON_API_KEY, defaulting to --use-public")
            args.use_public = True

    train_dataset, eval_dataset = trainer.prepare_dataset(
        dataset_path=args.dataset,
        use_public_data=args.use_public,
        polygon_df=polygon_df,
    )

    results = trainer.train(train_dataset, eval_dataset)

    logger.info(f"Final Results: {results}")
    logger.info(f"Model saved to: {args.output_dir}")


if __name__ == "__main__":
    main()
