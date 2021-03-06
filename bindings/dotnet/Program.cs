﻿using System;
using System.Text;
using System.Runtime.InteropServices;

namespace Sardine
{
    class SrdContext
    {
        [DllImport("sardine", CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr Srd_New(bool server);

        [DllImport("sardine", CallingConvention = CallingConvention.Cdecl)]
        public static extern int Srd_SetBlob(IntPtr handle, byte[] blobName, int blobNameSize, byte[] blobData, int blobDataSize);

        [DllImport("sardine", CallingConvention = CallingConvention.Cdecl)]
        public static extern int Srd_GetBlobName(IntPtr handle, byte[] data, int size);

        [DllImport("sardine", CallingConvention = CallingConvention.Cdecl)]
        public static extern int Srd_GetBlobData(IntPtr handle, byte[] data, int size);

        [DllImport("sardine", CallingConvention = CallingConvention.Cdecl)]
        public static extern int Srd_SetCertData(IntPtr handle, byte[] data, int size);

        [DllImport("sardine", CallingConvention = CallingConvention.Cdecl)]
        public static extern int Srd_GetDelegationKey(IntPtr handle, byte[] data, int size);

        [DllImport("sardine", CallingConvention = CallingConvention.Cdecl)]
        public static extern int Srd_GetIntegrityKey(IntPtr handle, byte[] data, int size);

        [DllImport("sardine", CallingConvention = CallingConvention.Cdecl)]
        public static extern int Srd_Input(IntPtr handle, byte[] data, int size);

        [DllImport("sardine", CallingConvention = CallingConvention.Cdecl)]
        public static extern int Srd_Output(IntPtr handle, byte[] data, int size);

        [DllImport("sardine", CallingConvention = CallingConvention.Cdecl)]
        public static extern void Srd_Free(IntPtr handle);

        private IntPtr m_handle;

        public SrdContext(bool server)
        {
            m_handle = Srd_New(server);
        }

        public int SetBlob(byte[] blobName, byte[] blobData)
        {
            return Srd_SetBlob(m_handle, blobName, blobName.Length, blobData, blobData.Length);
        }
        public int SetBlob(string blobName, byte[] blobData)
        {
            return SetBlob(StringToBytes(blobName, true), blobData);
        }

        public int SetBlob(string blobName, string blobData)
        {
            return SetBlob(blobName, StringToBytes(blobData, true));
        }

        public string GetBlobName()
        {
            int size;
            byte[] data;

            size = Srd_GetBlobName(m_handle, null, 0);

            if (size < 1)
                return "";

            data = new byte[size];
            size = Srd_GetBlobName(m_handle, data, size);

            UTF8Encoding utf8 = new UTF8Encoding();
            return utf8.GetString(data, 0, size - 1);
        }

        public int GetBlobData(ref string str)
        {
            int size;
            byte[] data;

            str = "";

            size = Srd_GetBlobData(m_handle, null, 0);

            if (size < 1)
                return size;

            data = new byte[size];
            size = Srd_GetBlobData(m_handle, data, size);

            UTF8Encoding utf8 = new UTF8Encoding();
            str = utf8.GetString(data, 0, size - 1);
            return size;
        }

        public int GetBlobData(ref byte[] data)
        {
            int size;

            data = null;

            size = Srd_GetBlobData(m_handle, null, 0);

            if (size < 1)
                return size;

            data = new byte[size];
            size = Srd_GetBlobData(m_handle, data, size);

            return size;
        }

        public int GetDelegationKey(ref byte[] data)
        {
            int size;

            data = null;

            size = Srd_GetDelegationKey(m_handle, null, 0);

            if (size < 1)
                return size;

            data = new byte[size];
            size = Srd_GetDelegationKey(m_handle, data, size);

            return size;
        }

        public int GetIntegrityKey(ref byte[] data)
        {
            int size;

            data = null;

            size = Srd_GetIntegrityKey(m_handle, null, 0);

            if (size < 1)
                return size;

            data = new byte[size];
            size = Srd_GetIntegrityKey(m_handle, data, size);

            return size;
        }

        public int SetCertData(byte[] data)
        {
            return Srd_SetCertData(m_handle, data, data != null ? data.Length : 0);
        }

        public int Input(byte[] inData)
        {
            return Srd_Input(m_handle, inData, inData != null ? inData.Length : 0);
        }

        public int Output(ref byte[] outData)
        {
            int outSize;

            outData = null;
            outSize = Srd_Output(m_handle, null, 0);

            if (outSize > 0)
            {
                outData = new byte[outSize];
                outSize = Srd_Output(m_handle, outData, outSize);
            }

            return outSize;
        }

        public int Authenticate(byte[] inData, ref byte[] outData)
        {
            int status;

            status = Input(inData);

            Output(ref outData);

            return status;
        }

        public static byte[] StringToBytes(string str, bool terminator)
        {
            UTF8Encoding utf8 = new UTF8Encoding();

            if (!terminator)
                return utf8.GetBytes(str);

            int byteCount = utf8.GetByteCount(str);
            byte[] nameBytes = new byte[byteCount + 1];
            utf8.GetBytes(str, 0, byteCount, nameBytes, 0);
            nameBytes[byteCount] = 0;
            
            return nameBytes;
        }

        ~SrdContext()
        {
            if (m_handle != IntPtr.Zero)
            {
                Srd_Free(m_handle);
                m_handle = IntPtr.Zero;
            }
        }
    }

    class Utils
    {
        public static void HexDump(byte[] bytes)
        {
            if ((bytes == null) || (bytes.Length == 0))
                return;

            for (int i = 0; i < bytes.Length; i++)
            {
                if ((i != 0) && ((i % 16) == 0))
                    Console.WriteLine("");

                Console.Write( "{0:X2} ", bytes[i]);
            }
            Console.WriteLine();
        }
    }

    class Program
    {
        static int test_sardine()
        {
            int messageNum = 1;
            int clientStatus = 1;
            int serverStatus = 1;
            byte[] inData = null;
            byte[] outData = null;
            SrdContext client = new SrdContext(false);
            SrdContext server = new SrdContext(true);

            client.SetBlob("Basic", "username:password");

            do
            {
                clientStatus = client.Authenticate(inData, ref outData);

                inData = null;

                if ((clientStatus <= 0) && (serverStatus != 1))
                    break;

                inData = outData;

                if (inData != null)
                {
                    Console.WriteLine("SRD Message #{0} ({1} bytes):", messageNum++, inData.Length);
                    Utils.HexDump(inData);
                    Console.WriteLine("");
                }

                serverStatus = server.Authenticate(inData, ref outData);

                inData = null;

                if ((serverStatus <= 0) && (clientStatus != 1))
                    break;

                inData = outData;

                if (inData != null)
                {
                    Console.WriteLine("SRD Message #{0} ({1} bytes):", messageNum++, inData.Length);
                    Utils.HexDump(inData);
                    Console.WriteLine("");
                }
            }
            while ((clientStatus == 1) || (serverStatus == 1));

            Console.WriteLine("client: {0} server: {1}", clientStatus, serverStatus);

            string blobData = "";
            server.GetBlobData(ref blobData);

            Console.WriteLine("BlobName: {0} BlobData: {1}",
                server.GetBlobName(), blobData);

            byte[] delegationKey = null;
            byte[] integrityKey = null;
            server.GetDelegationKey(ref delegationKey); // same as client, used for encryption
            server.GetIntegrityKey(ref integrityKey); // same as client, used for integrity

            Console.WriteLine("\nDelegationKey:");
            Utils.HexDump(delegationKey);
            Console.WriteLine("\nIntegrityKey:");
            Utils.HexDump(integrityKey);

            return 1;
        }
        static void Main(string[] args)
        {
            test_sardine();
        }
    }
}
