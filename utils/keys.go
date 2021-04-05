package utils

import (
	"crypto/rand"
	"crypto/rsa"
	"crypto/x509"
	"encoding/pem"
	"io/ioutil"
	"log"
)

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
