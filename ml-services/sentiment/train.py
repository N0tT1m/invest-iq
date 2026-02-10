"""FinBERT Fine-tuning Script."""
import torch
from transformers import (
    AutoTokenizer,
    AutoModelForSequenceClassification,
    Trainer,
    TrainingArguments,
    DataCollatorWithPadding
)
from datasets import Dataset, load_dataset
import numpy as np
from pathlib import Path
import argparse
import sys
from loguru import logger
from sklearn.metrics import accuracy_score, f1_score, precision_recall_fscore_support

sys.path.append(str(Path(__file__).parent.parent))
from shared.config import config
from shared.database import MLDatabase


class FinBERTTrainer:
    """Fine-tune FinBERT on custom financial sentiment data."""

    def __init__(
        self,
        model_name: str = "ProsusAI/finbert",
        output_dir: str = "./models/sentiment/fine-tuned",
        learning_rate: float = 2e-5,
        epochs: int = 3,
        batch_size: int = 16,
        warmup_steps: int = 500,
        weight_decay: float = 0.01
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

    def prepare_dataset(self, dataset_path: str = None, use_public_data: bool = True):
        """
        Prepare training dataset.

        Args:
            dataset_path: Path to custom CSV file with columns: text, label (0=positive, 1=negative, 2=neutral)
            use_public_data: Use publicly available financial sentiment datasets
        """
        if dataset_path:
            # Load custom dataset
            import pandas as pd
            df = pd.read_csv(dataset_path)
            dataset = Dataset.from_pandas(df)
            logger.info(f"Loaded custom dataset: {len(dataset)} samples")

        elif use_public_data:
            # Use Financial PhraseBank dataset (publicly available)
            logger.info("Loading Financial PhraseBank dataset...")
            dataset = load_dataset("financial_phrasebank", "sentences_allagree", split="train")

            # Map labels to FinBERT format
            # Financial PhraseBank: 0=negative, 1=neutral, 2=positive
            # FinBERT: 0=positive, 1=negative, 2=neutral
            def remap_labels(example):
                label_map = {0: 1, 1: 2, 2: 0}  # negative->negative, neutral->neutral, positive->positive
                example['label'] = label_map[example['label']]
                return example

            dataset = dataset.map(remap_labels)
            logger.info(f"Loaded public dataset: {len(dataset)} samples")

        else:
            raise ValueError("Must provide either dataset_path or set use_public_data=True")

        # Split dataset
        dataset = dataset.train_test_split(test_size=0.2, seed=42)
        train_dataset = dataset['train']
        eval_dataset = dataset['test']

        logger.info(f"Train samples: {len(train_dataset)}")
        logger.info(f"Eval samples: {len(eval_dataset)}")

        return train_dataset, eval_dataset

    def tokenize_dataset(self, dataset, tokenizer):
        """Tokenize dataset."""
        def tokenize_function(examples):
            # Handle both 'text' and 'sentence' column names
            text_column = 'text' if 'text' in examples else 'sentence'
            return tokenizer(
                examples[text_column],
                padding=False,
                truncation=True,
                max_length=512
            )

        tokenized = dataset.map(
            tokenize_function,
            batched=True,
            remove_columns=dataset.column_names
        )
        return tokenized

    def compute_metrics(self, eval_pred):
        """Compute evaluation metrics."""
        predictions, labels = eval_pred
        predictions = np.argmax(predictions, axis=1)

        accuracy = accuracy_score(labels, predictions)
        precision, recall, f1, _ = precision_recall_fscore_support(
            labels, predictions, average='weighted'
        )

        return {
            'accuracy': accuracy,
            'f1': f1,
            'precision': precision,
            'recall': recall
        }

    def train(self, train_dataset, eval_dataset):
        """Train the model."""
        logger.info("Starting training...")

        # Load tokenizer and model
        tokenizer = AutoTokenizer.from_pretrained(self.model_name)
        model = AutoModelForSequenceClassification.from_pretrained(
            self.model_name,
            num_labels=3
        )

        # Tokenize datasets
        train_tokenized = self.tokenize_dataset(train_dataset, tokenizer)
        eval_tokenized = self.tokenize_dataset(eval_dataset, tokenizer)

        # Data collator
        data_collator = DataCollatorWithPadding(tokenizer=tokenizer)

        # Training arguments
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

        # Initialize trainer
        trainer = Trainer(
            model=model,
            args=training_args,
            train_dataset=train_tokenized,
            eval_dataset=eval_tokenized,
            tokenizer=tokenizer,
            data_collator=data_collator,
            compute_metrics=self.compute_metrics
        )

        # Train
        logger.info("Training model...")
        train_result = trainer.train()

        # Evaluate
        logger.info("Evaluating model...")
        eval_result = trainer.evaluate()

        # Save model
        logger.info(f"Saving model to {self.output_dir}")
        trainer.save_model(str(self.output_dir))
        tokenizer.save_pretrained(str(self.output_dir))

        # Log metrics
        logger.info("Training complete!")
        logger.info(f"Training metrics: {train_result.metrics}")
        logger.info(f"Evaluation metrics: {eval_result}")

        # Save to database
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
                "batch_size": self.batch_size
            }
        )

        return eval_result


def main():
    parser = argparse.ArgumentParser(description="Fine-tune FinBERT on financial sentiment data")
    parser.add_argument("--dataset", type=str, help="Path to custom dataset CSV")
    parser.add_argument("--use-public", action="store_true", default=True,
                       help="Use public financial sentiment datasets")
    parser.add_argument("--output-dir", type=str, default="./models/sentiment/fine-tuned",
                       help="Output directory for trained model")
    parser.add_argument("--epochs", type=int, default=3, help="Number of training epochs")
    parser.add_argument("--batch-size", type=int, default=16, help="Training batch size")
    parser.add_argument("--learning-rate", type=float, default=2e-5, help="Learning rate")

    args = parser.parse_args()

    # Initialize trainer
    trainer = FinBERTTrainer(
        output_dir=args.output_dir,
        learning_rate=args.learning_rate,
        epochs=args.epochs,
        batch_size=args.batch_size
    )

    # Prepare dataset
    train_dataset, eval_dataset = trainer.prepare_dataset(
        dataset_path=args.dataset,
        use_public_data=args.use_public
    )

    # Train
    results = trainer.train(train_dataset, eval_dataset)

    logger.info(f"Final Results: {results}")
    logger.info(f"Model saved to: {args.output_dir}")


if __name__ == "__main__":
    main()
