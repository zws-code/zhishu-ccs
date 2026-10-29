// Opt-in signing smoke test. Only stores a dummy value in a uniquely named item.
#include <Security/Security.h>
#include <stdio.h>
#include <string.h>

#ifndef PROBE_VERSION
#define PROBE_VERSION 1
#endif

int main(int argc, char **argv) {
  const char *prefix = "com.zhishu.ccswitch.dev-signing-test.";
  const char *account = "probe";
  const char *value = "non-secret-signing-probe";
  if (argc != 3 || strncmp(argv[2], prefix, strlen(prefix)) != 0) return 2;
  OSStatus status = SecKeychainSetUserInteractionAllowed(false);
  if (status != errSecSuccess) return 3;
  if (strcmp(argv[1], "write") == 0) {
    status = SecKeychainAddGenericPassword(NULL, strlen(argv[2]), argv[2],
        strlen(account), account, strlen(value), value, NULL);
  } else {
    UInt32 length = 0;
    void *bytes = NULL;
    SecKeychainItemRef item = NULL;
    status = SecKeychainFindGenericPassword(NULL, strlen(argv[2]), argv[2],
        strlen(account), account, &length, &bytes, &item);
    if (status == errSecSuccess) {
      if (strcmp(argv[1], "delete") == 0) {
        status = SecKeychainItemDelete(item);
      } else if (strcmp(argv[1], "read") != 0 || length != strlen(value) ||
          memcmp(bytes, value, length) != 0) {
        status = errSecDecode;
      }
      SecKeychainItemFreeContent(NULL, bytes);
      CFRelease(item);
    }
  }
  printf("probe version=%d status=%d\n", PROBE_VERSION, (int)status);
  return status == errSecSuccess ? 0 : 1;
}
