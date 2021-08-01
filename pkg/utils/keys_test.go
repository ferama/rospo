package utils

import "testing"

func TestGenerateKeys(t *testing.T) {
	key, err := GeneratePrivateKey()
	if err != nil {
		t.Error(err)
	}

	EncodePrivateKeyToPEM(key)

	_, err = GeneratePublicKey(&key.PublicKey)
	if err != nil {
		t.Error(err)
	}
}
