package org.opentools.clouddrivebridge

import android.database.Cursor
import android.database.MatrixCursor
import android.os.CancellationSignal
import android.os.ParcelFileDescriptor
import android.provider.DocumentsContract
import android.provider.DocumentsProvider
import java.io.FileNotFoundException

/**
 * SCAFFOLD — v0.3, NOT registered in the shipped app (see README.md in
 * this folder). Buckets as SAF roots; every data call goes to cdb-core
 * through the UniFFI Kotlin bindings generated from ../../ffi
 * (org.opentools.cdbffi.CdbSession). Methods marked TODO(ffi) throw
 * until the generated Android project wires the bindings in — this
 * class must never ship in this state.
 */
class CdbDocumentsProvider : DocumentsProvider() {

    // TODO(ffi): private lateinit var session: CdbSession

    override fun onCreate(): Boolean {
        // TODO(ffi): session = CdbSession()
        // TODO(ffi): load profiles from the app store; secrets come from
        // Android Keystore and are handed to session.configure(...)
        return true
    }

    override fun queryRoots(projection: Array<out String>?): Cursor {
        val cursor = MatrixCursor(projection ?: DEFAULT_ROOT_PROJECTION)
        // TODO(ffi): one row per configured profile:
        //   Root.COLUMN_ROOT_ID = profile name
        //   Root.COLUMN_TITLE = "Cloud Drive Bridge"
        //   Root.COLUMN_FLAGS = FLAG_SUPPORTS_SEARCH or FLAG_SUPPORTS_CREATE
        //   Root.COLUMN_DOCUMENT_ID = root doc id for the profile
        return cursor
    }

    override fun queryDocument(documentId: String, projection: Array<out String>?): Cursor {
        // TODO(ffi): stat via session (cdb-core stat) -> one row.
        throw FileNotFoundException("scaffold: not wired yet")
    }

    override fun queryChildDocuments(
        parentDocumentId: String,
        projection: Array<out String>?,
        sortOrder: String?
    ): Cursor {
        // TODO(ffi): session.listJson(prefixOf(parentDocumentId)) -> rows.
        throw FileNotFoundException("scaffold: not wired yet")
    }

    override fun openDocument(
        documentId: String,
        mode: String,
        signal: CancellationSignal?
    ): ParcelFileDescriptor {
        // TODO(ffi): read mode -> ParcelFileDescriptor pipe fed from a
        // presigned URL stream or ranged reads; write mode -> v0.3.
        throw FileNotFoundException("scaffold: not wired yet")
    }

    companion object {
        private val DEFAULT_ROOT_PROJECTION = arrayOf(
            DocumentsContract.Root.COLUMN_ROOT_ID,
            DocumentsContract.Root.COLUMN_TITLE,
            DocumentsContract.Root.COLUMN_FLAGS,
            DocumentsContract.Root.COLUMN_DOCUMENT_ID
        )
    }
}
