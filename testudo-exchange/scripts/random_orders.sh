#!/bin/bash

# Function to generate random order data
generate_order_data() {
  # Randomly select BUY or SELL
  SIDE=$(shuf -e BUY SELL -n 1)
  
  # Generate random price between 15 and 22 for BTC_USDT (formatted to 2 decimal places)
  PRICE=$(awk -v min=95 -v max=98 'BEGIN{srand(); printf "%.2f", min+rand()*(max-min)}')
  
  # Generate random quantity between 0.01 and 0.05 (formatted to 4 decimal places)
  QUANTITY=$(awk -v min=0.01 -v max=0.05 'BEGIN{srand(); printf "%.4f", min+rand()*(max-min)}')
  
  # User id remains static
  USER_ID="test_user"
  
  # Create JSON data for the order
  ORDER_DATA=$(cat <<EOF
{
  "market": "SOL_USDC",
  "side": "$SIDE",
  "quantity": $QUANTITY,
  "price": $PRICE,
  "user_id": "$USER_ID"
}
EOF
)

  echo "$ORDER_DATA"
}

# Infinite loop to create random buy/sell orders
while true
do
  # Generate a random order
  ORDER=$(generate_order_data)

  # Submit the order using curl
  curl --location 'http://localhost:7000/api/v1/order' \
  --header 'Content-Type: application/json' \
  --data "$ORDER"

  # Wait for 2 to 5 seconds before the next order
  sleep 0.05
done
