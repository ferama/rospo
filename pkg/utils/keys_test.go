package utils

import (
	"io/ioutil"
	"log"
	"os"
	"testing"

	"golang.org/x/crypto/ssh"
)

func TestGenerateKeys(t *testing.T) {
	key, err := GeneratePrivateKey()
	if err != nil {
		t.Error(err)
	}

	EncodePrivateKeyToPEM(key)

	bytes, err := GeneratePublicKey(&key.PublicKey)
	if err != nil {
		t.Error(err)
	}
	file, err := ioutil.TempFile("", "testkey")
	if err != nil {
		log.Fatal(err)
	}
	defer os.Remove(file.Name())
	err = WriteKeyToFile(bytes, file.Name())
	if err != nil {
		t.Error(err)
	}

	os.Chmod(file.Name(), 0000)
	err = WriteKeyToFile(bytes, file.Name())
	if err == nil {
		t.Fail()
	}

	file, err = ioutil.TempFile("", "testkey")
	if err != nil {
		log.Fatal(err)
	}
	defer os.Remove(file.Name())
	pubkey, _ := ssh.NewPublicKey(&key.PublicKey)
	AddHostKeyToKnownHosts("testhost:2222", pubkey, file.Name())

	SerializePublicKey(pubkey)
}

func TestIdentity(t *testing.T) {
	id, err := LoadIdentityFile("testdata/identity")
	if id == nil || err != nil {
		t.Fail()
	}

	id, err = LoadIdentityFile("some-not-existent")
	if id != nil || err == nil {
		t.Fail()
	}
}
