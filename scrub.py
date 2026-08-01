import sys
import os
from presidio_analyzer import AnalyzerEngine, PatternRecognizer, Pattern
from presidio_anonymizer import AnonymizerEngine

DB_KEY = [
    Pattern("db_url_env", r'(?:DATABASE_URL|DB_URL|MONGO_URI|REDIS_URL)\s*=\s*\S+', 0.95),
    Pattern("db_password", r'(?:password|passwd|pwd)\s*[:=]\s*\S+', 0.9),
    Pattern("conn_string", r'(?:mongodb|postgres(?:ql)?|mysql|redis)://[^\s\'"]+', 1.0),
]

DEV_ID = [
    Pattern("api_key", r'(?:API_KEY|apiKey|SECRET_KEY|secretKey|AUTH_TOKEN)\s*[:=]\s*\S+', 1.0),
    Pattern("dev_id", r'(?:dev_id|developer_id)\s*[:=]\s*\S+', 0.85),
]


def scrub(path):
    try:
        with open(path) as f:
            text = f.read()
    except Exception:
        return False

    analyzer = AnalyzerEngine()
    analyzer.registry.add_recognizer(
        PatternRecognizer(supported_entity="DB_KEY", patterns=DB_KEY)
    )
    analyzer.registry.add_recognizer(
        PatternRecognizer(supported_entity="DEV_ID", patterns=DEV_ID)
    )

    results = analyzer.analyze(text=text, language="en")
    if not results:
        return False

    redacted = AnonymizerEngine().anonymize(text=text, analyzer_results=results)
    with open(path, "w") as f:
        f.write(redacted.text)
    return True


def main():
    changed = False
    for p in sys.argv[1:]:
        if os.path.isfile(p) and scrub(p):
            changed = True
    sys.exit(2 if changed else 0)


if __name__ == "__main__":
    main()
