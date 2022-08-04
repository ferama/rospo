package cmd

import (
	"fmt"
	"io"
	"io/fs"
	"log"
	"os"
	"path/filepath"
	"strings"

	"github.com/ferama/rospo/cmd/cmnflags"
	"github.com/ferama/rospo/pkg/sshc"
	"github.com/pkg/sftp"
	"github.com/spf13/cobra"
)

func init() {
	rootCmd.AddCommand(putCmd)

	cmnflags.AddSshClientFlags(putCmd.Flags())
	putCmd.Flags().BoolP("recursive", "r", false, "if the copy should be recursive")
}

func putFile(client *sftp.Client, remote, local string) error {
	remotePath, err := client.RealPath(remote)
	if err != nil {
		return fmt.Errorf("invalid remote path: %s", remotePath)
	}
	remoteStat, err := client.Stat(remotePath)
	if err == nil && remoteStat.IsDir() {
		remotePath = filepath.Join(remotePath, filepath.Base(local))
	}

	stat, err := os.Stat(local)
	if err != nil {
		return fmt.Errorf("cannot stat local path: %s", local)
	}
	lFile, err := os.Open(local)
	if err != nil {
		return fmt.Errorf("cannot open local file for read: %s", err)
	}
	defer lFile.Close()

	rFile, err := client.Create(remotePath)
	if err != nil {
		return fmt.Errorf("cannot open remote file for write: %s", err)
	}
	defer rFile.Close()

	nBytes, err := io.Copy(rFile, lFile)
	if err != nil {
		return fmt.Errorf("error while writing remote file: %s", err)
	}
	log.Printf("%d byte written", nBytes)

	rFile.Chmod(stat.Mode())
	return nil
}

func putFileRecursive(client *sftp.Client, remote, local string) error {
	remotePath, err := client.RealPath(remote)
	if err != nil {
		return fmt.Errorf("invalid remote path: %s", remotePath)
	}

	localStat, err := os.Stat(local)
	if err != nil {
		return fmt.Errorf("cannot stat local path: %s", local)
	}
	if !localStat.IsDir() {
		return fmt.Errorf("local path is not a directory: %s", local)
	}

	remoteStat, err := client.Stat(remotePath)
	if err != nil {
		return fmt.Errorf("cannot stat remote path: %s", remotePath)
	}
	if !remoteStat.IsDir() {
		return fmt.Errorf("local path is not a directory: %s", remotePath)
	}

	dir := filepath.Base(local)
	err = filepath.WalkDir(local, func(localPath string, d fs.DirEntry, err error) error {
		part := strings.TrimPrefix(localPath, local)
		targetPath := filepath.Join(remotePath, dir, part)
		log.Println(localPath, targetPath, d.IsDir())
		if d.IsDir() {
			err := client.Mkdir(targetPath)
			if err != nil {
				return fmt.Errorf("cannot create directory %s: %s", remotePath, err)
			}
		} else {
			log.Printf("copying from local: %s, to remote: %s", localPath, targetPath)
			err := putFile(client, targetPath, localPath)
			if err != nil {
				return err
			}
		}
		return nil
	})
	if err != nil {
		log.Println(err)
	}
	return nil
}

var putCmd = &cobra.Command{
	Use:   "put [user@]host[:port] local [remote]",
	Short: "puts files from local to remote",
	Long:  "puts files from local to remote",
	Args:  cobra.MinimumNArgs(3),
	Run: func(cmd *cobra.Command, args []string) {
		local := args[1]
		remote := args[2]
		recursive, _ := cmd.Flags().GetBool("recursive")
		sshcConf := cmnflags.GetSshClientConf(cmd, args[0])
		conn := sshc.NewSshConnection(sshcConf)
		go conn.Start()
		conn.Connected.Wait()

		client, err := sftp.NewClient(conn.Client)
		if err != nil {
			log.Fatal(err)
		}
		defer client.Close()

		if recursive {
			err = putFileRecursive(client, remote, local)
		} else {
			err = putFile(client, remote, local)
		}
		if err != nil {
			log.Fatalln(err)
		}

	},
}
