package utils

import (
	"crypto/rand"
	"crypto/rsa"
	"crypto/x509"
	"encoding/pem"
	"fmt"
	"io/ioutil"
	"log"

	"golang.org/x/crypto/ssh"
)

// GeneratePrivateKey generate an rsa key (actually used from the sshd server)
func GeneratePrivateKey(keyPath *string) {
	bitSize := 4096
	privateKey, err := rsa.GenerateKey(rand.Reader, bitSize)
	if err != nil {
		log.Println(err)
	}

	// Validate Private Key
	err = privateKey.Validate()
	if err != nil {
		log.Println(err)
	}

	log.Println("Private Key generated")

	privDER := x509.MarshalPKCS1PrivateKey(privateKey)

	// pem.Block
	privBlock := pem.Block{
		Type:    "RSA PRIVATE KEY",
		Headers: nil,
		Bytes:   privDER,
	}

	// Private key in PEM format
	privatePEM := pem.EncodeToMemory(&privBlock)
	if err := ioutil.WriteFile(*keyPath, privatePEM, 0600); err != nil {
		log.Println(err)
	}

	log.Printf("Key saved to: %s", *keyPath)
}

// PublicKeyFile reads a public key file and loads the keys to
// an ssh.PublicKeys object
func PublicKeyFile(file string) ssh.AuthMethod {
	buffer, err := ioutil.ReadFile(file)
	if err != nil {
		log.Fatalln(fmt.Sprintf("Cannot read SSH public key file %s", file))
		return nil
	}

	key, err := ssh.ParsePrivateKey(buffer)
	if err != nil {
		log.Fatalln(fmt.Sprintf("Cannot parse SSH public key file %s", file))
		return nil
	}
	return ssh.PublicKeys(key)
}
