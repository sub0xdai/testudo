import sys
import os
from presidio_analyzer import AnalyzerEngine, PatternRecognizer, Pattern
from presidio_anonymizer import AnonymizerEngine

DB_KEY = [
    Pattern("db_url_env", r'(?:DATABASE_URL|DB_URL|MONGO_URI|REDIS_URL)\s*=\s*\S+', 0.95),
    Pattern("db_password", r'(?i)(?:password|passwd|pwd)\s*[:=]\s*["\x27]([^"\x27]{8,})["\x27]', 0.9),
    Pattern("conn_string", r'(?:mongodb|postgres(?:ql)?|mysql|redis)://[^\s\'"]+', 1.0),
]

DEV_ID = [
    # Match secrets in config files: KEY = "actual-secret-value" or KEY=hexstring
    Pattern("secret_assignment", r'(?i)(?:api_key|secret_key|auth_token|private_key)\s*[:=]\s*["\x27]([^"\x27]{16,})["\x27]', 1.0),
    Pattern("secret_env", r'(?i)(?:api_key|secret_key|auth_token|private_key)\s*[:=]\s*[A-Za-z0-9+/=]{32,}', 0.95),
]


def scrub(path):
    try:
        with open(path) as f:
            text = f.read()
    except Exception:
        return False

    # Create analyzer without built-in recognizers (they match code identifiers as PII).
    from presidio_analyzer import RecognizerRegistry
    registry = RecognizerRegistry()
    registry.add_recognizer(
        PatternRecognizer(supported_entity="DB_KEY", patterns=DB_KEY)
    )
    registry.add_recognizer(
        PatternRecognizer(supported_entity="DEV_ID", patterns=DEV_ID)
    )
    analyzer = AnalyzerEngine(registry=registry)

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
