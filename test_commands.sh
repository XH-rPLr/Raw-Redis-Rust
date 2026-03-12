#!/bin/bash

# 1. Test PING with NO arguments (Should return +PONG)
echo "--- Testing PING (No Args) ---"
printf "*1\r\n\$4\r\nPING\r\n" | nc localhost 6379

# 2. Test PING with an argument (Should return the argument as a Bulk String)
echo -e "\n--- Testing PING with message ---"
printf "*2\r\n\$4\r\nPING\r\n\$5\r\nhello\r\n" | nc localhost 6379

# 3. Test SET with EX (Seconds) - 10 seconds
echo -e "\n--- Testing SET with EX ---"
printf "*5\r\n\$3\r\nSET\r\n\$4\r\nuser\r\n\$4\r\nalex\r\n\$2\r\nEX\r\n:10\r\n" | nc localhost 6379

# 4. Test SET with PX (Milliseconds) - 500ms
echo -e "\n--- Testing SET with PX ---"
printf "*5\r\n\$3\r\nSET\r\n\$4\r\ntemp\r\n\$3\r\nval\r\n\$2\r\nPX\r\n:500\r\n" | nc localhost 6379

# 5. Test RPUSH with multiple values (Variadic)
echo -e "\n--- Testing RPUSH Variadic ---"
printf "*5\r\n\$5\r\nRPUSH\r\n\$4\r\ntags\r\n\$4\r\nrust\r\n\$5\r\nredis\r\n\$4\r\nfast\r\n" | nc localhost 6379

# 6. Test ECHO (Should return the message)
echo -e "\n--- Testing ECHO ---"
printf "*2\r\n\$4\r\nECHO\r\n\$11\r\ntest_passed\r\n" | nc localhost 6379

#7. Test SET
echo -e "\n--- Testing Transactional Multi-SET ---"
printf "*1\r\n\$5\r\nMULTI\r\n*3\r\n\$3\r\nSET\r\n\$4\r\nkey1\r\n\$4\r\nval1\r\n*3\r\n\$3\r\nSET\r\n\$4\r\nkey2\r\n\$4\r\nval2\r\n*1\r\n\$4\r\nEXEC\r\n" | nc localhost 6379

#8. Test DEL
echo -e "\n--- Testing DEL (Bulk Delete) ---"
printf "*4\r\n\$3\r\nDEL\r\n\$4\r\nkey1\r\n\$4\r\nkey2\r\n\$4\r\nuser\r\n" | nc localhost 6379

# 9. Test GET to verify MULTI-SET results
echo -e "\n--- Verifying MULTI-SET Results ---"
printf "*2\r\n\$3\r\nGET\r\n\$4\r\nkey1\r\n*2\r\n\$3\r\nGET\r\n\$4\r\nkey2\r\n" | nc localhost 6379

# 10. Test UNLINK (Single Key)
echo -e "\n--- Testing UNLINK (Single Key) ---"
# Sets a key, then unlinks it
printf "*3\r\n\$3\r\nSET\r\n\$7\r\nlink_me\r\n\$4\r\ndata\r\n" | nc localhost 6379
printf "*2\r\n\$6\r\nUNLINK\r\n\$7\r\nlink_me\r\n" | nc localhost 6379

# 11. Test UNLINK (Multiple Keys & Non-existent)
echo -e "\n--- Testing UNLINK (Variadic & Missing) ---"
# We'll try to unlink keys from previous tests and one fake one
printf "*4\r\n\$6\r\nUNLINK\r\n\$4\r\nkey1\r\n\$4\r\nkey2\r\n\$11\r\nnon_existent\r\n" | nc localhost 6379

# 12. Final Verification
echo -e "\n--- Verifying UNLINK Results (Should be nil) ---"
printf "*2\r\n\$3\r\nGET\r\n\$7\r\nlink_me\r\n" | nc localhost 6379

# 13. Test FLUSHALL
echo -e "\n--- Testing FLUSHALL (Case Insensitivity & Verification) ---"
# Seed a canary key
printf "*3\r\n\$3\r\nSET\r\n\$6\r\ncanary\r\n\$5\r\nalive\r\n" | nc localhost 6379
# Flush the database (using mixed case to test normalization)
printf "*1\r\n\$8\r\nFLUSHALL\r\n" | nc localhost 6379
# Verify the canary is gone
printf "*2\r\n\$3\r\nGET\r\n\$6\r\ncanary\r\n" | nc localhost 6379